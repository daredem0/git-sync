use ratatui::Frame;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

pub(crate) fn render_and_capture_text(
    width: u16,
    height: u16,
    draw: impl FnOnce(&mut Frame<'_>),
) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("must create test terminal");
    terminal.draw(draw).expect("render should succeed");
    let backend = terminal.backend();
    let mut output = String::new();
    let area = backend.buffer().area;
    for y in 0..area.height {
        for x in 0..area.width {
            let cell = backend
                .buffer()
                .cell((x, y))
                .expect("rendered cell should exist");
            output.push_str(cell.symbol());
        }
        output.push('\n');
    }
    output
}
