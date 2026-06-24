use std::io::{self, Stdout};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Cell, Paragraph, Row, Table},
};

use crate::repo::{Repo, State};
use crate::scan::{ScanOpts, scan};

const TITLE: [&str; 3] = [
    "╺┳╸┏━┓╻  ┏━┓┏━┓",
    " ┃ ┣━┫┃  ┃ ┃┗━┓",
    " ╹ ╹ ╹┗━╸┗━┛┗━┛",
];

// 3 title lines + blank + summary + blank + path
const HEADER_HEIGHT: u16 = 7;

const POLL: Duration = Duration::from_millis(250);

pub struct AppOpts {
    pub target: PathBuf,
    pub no_fetch: bool,
    pub fetch_ttl: Duration,
    pub refresh_interval: Duration,
}

pub fn run(opts: AppOpts) -> io::Result<()> {
    install_panic_hook();
    let mut terminal = enter()?;
    let result = event_loop(&mut terminal, opts);
    leave(&mut terminal)?;
    result
}

type Term = Terminal<CrosstermBackend<Stdout>>;

fn enter() -> io::Result<Term> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    Terminal::new(CrosstermBackend::new(stdout))
}

fn leave(terminal: &mut Term) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn install_panic_hook() {
    let original = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original(info);
    }));
}

struct ScanResult {
    repos: Vec<Repo>,
    duration: Duration,
}

struct App {
    opts: AppOpts,
    repos: Vec<Repo>,
    scanning: bool,
    last_scan_at: Option<Instant>,
    last_scan_duration: Option<Duration>,
    next_auto_scan: Instant,
    tx: Sender<ScanResult>,
    rx: Receiver<ScanResult>,
}

impl App {
    fn new(opts: AppOpts) -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            next_auto_scan: Instant::now(),
            scanning: false,
            last_scan_at: None,
            last_scan_duration: None,
            repos: Vec::new(),
            opts,
            tx,
            rx,
        }
    }

    fn request_scan(&mut self, force_fetch: bool) {
        if self.scanning {
            return;
        }
        self.scanning = true;
        let target = self.opts.target.clone();
        let scan_opts = ScanOpts {
            fetch: !self.opts.no_fetch,
            fetch_ttl: self.opts.fetch_ttl,
            force_fetch,
        };
        let tx = self.tx.clone();
        thread::spawn(move || {
            let started = Instant::now();
            let repos = scan(&target, &scan_opts);
            let _ = tx.send(ScanResult {
                repos,
                duration: started.elapsed(),
            });
        });
    }

    fn drain_results(&mut self) {
        while let Ok(r) = self.rx.try_recv() {
            self.repos = r.repos;
            self.last_scan_at = Some(Instant::now());
            self.last_scan_duration = Some(r.duration);
            self.scanning = false;
            self.next_auto_scan = Instant::now() + self.opts.refresh_interval;
        }
    }
}

fn event_loop(terminal: &mut Term, opts: AppOpts) -> io::Result<()> {
    let mut app = App::new(opts);
    app.request_scan(false);

    loop {
        app.drain_results();
        terminal.draw(|f| draw(f, &app))?;

        if event::poll(POLL)? {
            match event::read()? {
                Event::Key(k) if k.kind == KeyEventKind::Press => match k.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                        return Ok(());
                    }
                    KeyCode::Char('r') => app.request_scan(false),
                    KeyCode::Char('f') => app.request_scan(true),
                    _ => {}
                },
                _ => {}
            }
        }

        if Instant::now() >= app.next_auto_scan {
            app.request_scan(false);
        }
    }
}

fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::vertical([
        Constraint::Length(HEADER_HEIGHT),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(f.area());

    draw_header(f, chunks[0], app);
    draw_table(f, chunks[1], app);
    draw_footer(f, chunks[2], app);
}

struct Counts {
    push: usize,
    pull: usize,
    dirty: usize,
    no_upstream: usize,
    total: usize,
}

fn counts(repos: &[Repo]) -> Counts {
    Counts {
        push: repos
            .iter()
            .filter(|r| matches!(r.state, State::Push | State::Diverged))
            .count(),
        pull: repos
            .iter()
            .filter(|r| matches!(r.state, State::Pull | State::Diverged))
            .count(),
        dirty: repos.iter().filter(|r| r.dirty).count(),
        no_upstream: repos
            .iter()
            .filter(|r| r.state == State::NoUpstream)
            .count(),
        total: repos.iter().filter(|r| r.state != State::NotRepo).count(),
    }
}

fn count_chip(label: &str, color: Color, n: usize, total: usize, trailing_gap: bool) -> Vec<Span<'static>> {
    let gap = if trailing_gap { "  " } else { "" };
    vec![
        Span::styled(format!("{label}: "), Style::new().fg(color)),
        Span::raw(format!("{n}/{total}{gap}")),
    ]
}

fn draw_header(f: &mut Frame, area: Rect, app: &App) {
    let c = counts(&app.repos);
    let mut spans = Vec::with_capacity(8);
    spans.extend(count_chip("Pull", Color::Yellow, c.pull, c.total, true));
    spans.extend(count_chip("Push", Color::Red, c.push, c.total, true));
    spans.extend(count_chip("Dirty", Color::Magenta, c.dirty, c.total, true));
    spans.extend(count_chip("No upstream", Color::Cyan, c.no_upstream, c.total, false));

    let lines = vec![
        Line::from(TITLE[0]),
        Line::from(TITLE[1]),
        Line::from(TITLE[2]),
        Line::from(""),
        Line::from(spans),
        Line::from(""),
        Line::from(format!("PATH: {}", app.opts.target.display()).bold()),
    ];
    f.render_widget(Paragraph::new(lines), area);
}

fn draw_table(f: &mut Frame, area: Rect, app: &App) {
    let header = Row::new(["#", "repo", "branch", "state", "+/-", "last", "Σ"])
        .style(Style::new().add_modifier(Modifier::BOLD | Modifier::UNDERLINED));

    let rows = app.repos.iter().enumerate().map(|(i, r)| {
        let plusminus = if r.state == State::NotRepo {
            "-".into()
        } else {
            format!("{}/{}", r.ahead, r.behind)
        };
        let row_style = if r.dirty {
            Style::new().fg(Color::Magenta)
        } else {
            Style::new().fg(r.state.color())
        };
        Row::new(vec![
            Cell::from((i + 1).to_string()),
            Cell::from(r.name.clone()),
            Cell::from(r.branch.clone()),
            Cell::from(r.state.label()),
            Cell::from(plusminus),
            Cell::from(r.last_commit.clone()),
            Cell::from(r.commits.to_string()),
        ])
        .style(row_style)
    });

    let widths = [
        Constraint::Length(4),
        Constraint::Min(20),
        Constraint::Length(18),
        Constraint::Length(11),
        Constraint::Length(8),
        Constraint::Length(8),
        Constraint::Length(6),
    ];

    let table = Table::new(rows, widths).header(header).column_spacing(2);
    f.render_widget(table, area);
}

fn draw_footer(f: &mut Frame, area: Rect, app: &App) {
    let mut spans = vec![
        "q".bold(),
        Span::raw(" quit  "),
        "r".bold(),
        Span::raw(" rescan  "),
        "f".bold(),
        Span::raw(" force-fetch    "),
    ];

    if app.scanning {
        spans.push(Span::styled("scanning…", Style::new().fg(Color::Yellow)));
    } else if let Some(d) = app.last_scan_duration {
        let ago = app.last_scan_at.map_or(0, |t| t.elapsed().as_secs());
        spans.push(Span::raw(format!(
            "last scan: {:.2}s · {}s ago",
            d.as_secs_f32(),
            ago
        )));
    }

    f.render_widget(Paragraph::new(Line::from(spans)), area);
}
