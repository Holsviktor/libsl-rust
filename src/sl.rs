extern crate libc;
extern crate getopts;

use getopts::Options;

use crate::d51::SL;
use crate::c51::C51;
use crate::logo::Logo;
use crate::tgv::TGV;
pub mod d51;
pub mod c51;
pub mod logo;
pub mod tgv;
mod data;

use std::io::{self, Write};
use std::time::Duration;
use crossterm::{
    cursor,
    execute, queue,
    style::{self, Color},
    terminal::{self, enable_raw_mode, disable_raw_mode, ClearType},
    event,
};

pub trait Train {
    /// Approximate speed in km/h
    fn speed(&self) -> u32 {
        100
    }
    fn body(&self) -> &'static [&'static str];
    fn wheelset(&self, x: usize) -> &'static [&'static str];
    fn tender(&self) -> Option<&'static [&'static str]> {
        None
    }
    fn wagons(&self) -> u32 {
        0
    }
    fn wagon(&self) -> Option<&'static [&'static str]> {
        None
    }
}

fn speed2delay(speed: u32) -> Duration {
    // if 4_000_000: 100 km/h -> 40 ms
    Duration::from_micros((4_000_000 / speed) as u64)
}

trait Render: Train {
    fn render(&self, x: i32, stdout: &mut impl Write, cols: i32, lines: i32, use_color: bool) {
        let mut len = 0i32;
        let y = lines / 2;
        let body_iter = self.body().iter();
        let wheelset_iter = self.wheelset(x as usize).iter();
        let iter = body_iter.chain(wheelset_iter);
        let (_, hint) = iter.size_hint();
        let height = match hint {
            Some(s) => s,
            None => panic!("this really shouldn't happen"),
        };
        let offset = (height / 2) as i32;
        for (index, line) in iter.rev().enumerate() {
            if line.len() as i32 > len {
                len = line.len() as i32;
            }
            self.render_line((y + offset) - index as i32, x, *line, stdout, cols, use_color);
        }
        if let Some(tender) = self.tender() {
            let mut new_len = 0i32;
            for (index, line) in tender.iter().rev().enumerate() {
                if len + line.len() as i32 > new_len {
                    new_len = len + line.len() as i32;
                }
                self.render_line((y + offset) - index as i32, x + len, *line, stdout, cols, use_color);
            }
            len = new_len;
        }
        if let Some(wagon) = self.wagon() {
            for _ in 0..self.wagons() {
                let mut new_len = 0i32;
                for (index, line) in wagon.iter().rev().enumerate() {
                    if len + line.len() as i32 > new_len {
                        new_len = len + line.len() as i32;
                    }
                    self.render_line((y + offset) - index as i32, x + len, *line, stdout, cols, use_color);
                }
                len = new_len;
            }
        }
    }

    fn render_line(&self, y: i32, x: i32, line: &str, stdout: &mut impl Write, cols: i32, use_color: bool) {
        // Skip lines outside the vertical bounds of the terminal
        if y < 0 {
            return;
        }
        if x >= cols {
            return; 
        }

        let segment: Option<&str> = if x >= 0 {
            let paint_len = (cols - x) as usize;
            if paint_len == 0 {
                None
            } else if paint_len < line.len() {
                Some(&line[0..paint_len])
            } else {
                Some(line)
            }
        } else {
            let skip = (-x) as usize;
            if skip < line.len() {
                Some(&line[skip..])
            } else {
                None
            }
        };

        if let Some(text) = segment {
            let draw_x = x.max(0) as u16;
            let draw_y = y as u16;
            if use_color {
                queue!(
                    stdout,
                    cursor::MoveTo(draw_x, draw_y),
                    style::SetForegroundColor(Color::Yellow),
                    style::Print(text),
                    style::ResetColor,
                ).unwrap();
            } else {
                queue!(
                    stdout,
                    cursor::MoveTo(draw_x, draw_y),
                    style::Print(text),
                ).unwrap();
            }
        }
    }
}

impl Render for dyn Train {}
impl Render for SL {}
impl Render for C51 {}
impl Render for Logo {}
impl Render for TGV {}

pub fn sl(args: &[String]) {
    use libc::signal;
    use libc::SIGINT;
    use libc::SIG_IGN;

    let mut opts = Options::new();
    opts.optflag("l", "", "logo");
    opts.optflag("c", "", "C51");
    opts.optflag("G", "", "TGV");
    opts.optflag("a", "", "reserved for future use");
    opts.optflag("f", "", "reserved for future use");

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => panic!("{}", f.to_string()),
    };

    let use_color = matches.opt_present("G");

    enable_raw_mode().unwrap();

    let mut stdout = io::stdout();

    execute!(
        stdout,
        terminal::EnterAlternateScreen,
        cursor::Hide,
    ).unwrap();

    unsafe {
        signal(SIGINT, SIG_IGN);
    }

    let train: Box<dyn Train> = {
        if matches.opt_present("l") {
            Box::new(Logo)
        } else if matches.opt_present("c") {
            Box::new(C51)
        } else if matches.opt_present("G") {
            Box::new(TGV)
        } else {
            Box::new(SL)
        }
    };

    let (cols, lines) = terminal::size()
        .map(|(c, l)| (c as i32, l as i32))
        .unwrap_or((80, 24));

    let delay = speed2delay(train.speed());

    for x in (-85..cols).rev() {
        // Clear screen
        queue!(stdout, terminal::Clear(ClearType::All)).unwrap();

        train.render(x, &mut stdout, cols, lines, use_color);

        stdout.flush().unwrap();

        // Consume any pending key events (mirrors ncurses getch() with nodelay)
        if event::poll(Duration::ZERO).unwrap_or(false) {
            let _ = event::read();
        }

        std::thread::sleep(delay);
    }

    execute!(
        stdout,
        cursor::Show,
        terminal::LeaveAlternateScreen,
    ).unwrap();

    disable_raw_mode().unwrap();
}
