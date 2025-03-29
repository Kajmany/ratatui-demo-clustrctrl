use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Stylize,
    symbols::border,
    text::Line,
    widgets::{Block, Paragraph, Widget},
    DefaultTerminal, Frame,
};
use std::io;

fn main() -> Result<(), io::Error> {
    //color_eyre::install()?;
    let mut terminal = ratatui::init();
    let result = App::default().run(&mut terminal);
    ratatui::restore();
    result
}

#[derive(Debug, Default)]
pub struct App {
    exit: bool,
}

impl App {
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.view(frame))?;
            self.update()?;
        }
        Ok(())
    }

    fn view(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

    fn update(&mut self) -> io::Result<()> {
        match event::read()? {
            Event::Key(event) if event.kind == KeyEventKind::Press => self.handle_key_event(event),
            _ => {}
        };
        Ok(())
    }

    fn handle_key_event(&mut self, event: KeyEvent) {
        match event.code {
            KeyCode::Char('q') => {
                todo!()
            }
            KeyCode::Char('k') => {
                todo!()
            }
            KeyCode::Char('j') => {
                todo!()
            }
            KeyCode::F(1) => {
                todo!()
            }
            KeyCode::F(2) => {
                todo!()
            }
            KeyCode::F(3) => self.exit(),
            _ => {}
        }
    }

    fn exit(&mut self) {
        self.exit = true;
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = Line::from("  clustrctrl  ".bold());
        let controls = Line::from(vec![
            " New Task ".into(),
            "<F1>".blue().bold(),
            " Monitor ".into(),
            "<F2>".blue().bold(),
            " Quit ".into(),
            "<F3> ".blue().bold(),
        ]);
        /*
        let nav_controls = Line::from(vec![
            " Kill Task ".into(),
            "<Q>".blue().bold(),
            " Up ".into(),
            "<J>".blue().bold(),
            " Down ".into(),
            "<K>".blue().bold(),
        ]);
        */
        let block = Block::bordered()
            .title(title.left_aligned())
            .title_bottom(controls.centered())
            .border_set(border::THICK);

        Paragraph::new("Henlo!")
            .centered()
            .block(block)
            .render(area, buf);
    }
}
