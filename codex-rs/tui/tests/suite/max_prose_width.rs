//! Real CLI screen verification with isolated settings and local model responses.
use super::PtyCodex;
use super::write_test_config;
use anyhow::Result;
use anyhow::ensure;
use core_test_support::responses;
use std::os::fd::AsRawFd;
use std::time::Duration;
use wiremock::matchers::body_string_contains;

fn resize(terminal: &mut PtyCodex, width: u16) -> Result<()> {
    let size = libc::winsize {
        ws_row: 32,
        ws_col: width,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // SAFETY: the live PTY fd and initialized winsize remain valid during ioctl.
    ensure!(unsafe { libc::ioctl(terminal.master.as_raw_fd(), libc::TIOCSWINSZ, &size) } == 0);
    terminal.parser.screen_mut().set_size(32, width);
    // SAFETY: this PID belongs to the test's live CLI child; SIGWINCH only requests redraw.
    ensure!(unsafe { libc::kill(terminal.child.id() as libc::pid_t, libc::SIGWINCH) } == 0);
    for _ in 0..10 {
        terminal.read_output(Duration::from_millis(100))?;
    }
    terminal.ensure_running()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn max_prose_width_cli_screen_and_resume() -> Result<()> {
    let workspace = tempfile::tempdir()?;
    let cwd = workspace.path().canonicalize()?;
    let text = format!("{}\n\nPROSE_DONE", "PROSE한글 English ".repeat(24));
    for fullscreen in [true, false] {
        for limit in [None, Some(80), Some(100)] {
            let home = tempfile::tempdir()?;
            let home_path = home.path().to_owned();
            write_test_config(&home_path, &cwd)?;
            let server = responses::start_mock_server().await;
            let config_path = home_path.join("config.toml");
            let config = std::fs::read_to_string(&config_path)?
                .replace("model_provider = \"openai\"", "model_provider = \"test\"");
            let width_setting = limit
                .map(|limit| format!("max_prose_width = {limit}\n"))
                .unwrap_or_default();
            std::fs::write(
                &config_path,
                format!(
                    "{config}\n[model_providers.test]\nname = \"Mock\"\nbase_url = \"{}/v1\"\nwire_api = \"responses\"\nrequires_openai_auth = false\nsupports_websockets = false\n[tui]\nfullscreen_transcript = {fullscreen}\nanimations = false\nshow_tooltips = false\n{width_setting}",
                    server.uri(),
                ),
            )?;
            let _title = responses::mount_sse_once_match(
                &server,
                body_string_contains(r#"\"thread_source\":\"system\""#),
                responses::sse(vec![
                    responses::ev_assistant_message("title", r#"{"title":"Prose test"}"#),
                    responses::ev_completed("title-response"),
                ]),
            )
            .await;
            let _reply = responses::mount_sse_once_match(
                &server,
                body_string_contains(r#"\"thread_source\":\"user\""#),
                responses::sse(vec![
                    responses::ev_assistant_message("reply", &text),
                    responses::ev_completed("reply-response"),
                ]),
            )
            .await;
            let mut terminal = PtyCodex::start_cli(
                &cwd,
                home,
                if fullscreen {
                    &[]
                } else {
                    &["--no-alt-screen"]
                },
            )?;
            terminal.wait_for_startup()?;
            resize(&mut terminal, 160)?;
            terminal.write_input(b"Render prose")?;
            terminal.wait_for_screen("Render prose")?;
            // Bulk PTY input follows the composer's existing paste timing policy.
            std::thread::sleep(Duration::from_millis(250));
            terminal.write_input(b"\r")?;
            terminal.wait_for_screen("PROSE_DONE")?;
            let mut wide_rows: Option<Vec<String>> = None;
            for (stage, width) in [160, 60, 160].into_iter().enumerate() {
                resize(&mut terminal, width)?;
                terminal.wait_for_screen("PROSE_DONE")?;
                wait_for_prose(
                    &mut terminal,
                    if stage == 2 {
                        wide_rows.as_deref()
                    } else {
                        None
                    },
                )?;
                if stage == 0 {
                    wide_rows = Some(prose_rows(&terminal));
                }
                let screen = terminal.screen_contents();
                if let Some(directory) = std::env::var_os("CODEX_PROSE_SCREEN_DIR") {
                    let directory = std::path::PathBuf::from(directory);
                    std::fs::create_dir_all(&directory)?;
                    std::fs::write(
                        directory.join(format!(
                            "fullscreen-{fullscreen}-limit-{limit:?}-width-{width}.txt"
                        )),
                        &screen,
                    )?;
                }
                let rows = prose_rows(&terminal);
                ensure!(!rows.is_empty(), "missing prose: {screen}");
                if let Some(limit) = limit {
                    for line in rows {
                        ensure!(
                            line.chars()
                                .map(|ch| if ('\u{ac00}'..='\u{d7a3}').contains(&ch) {
                                    2
                                } else {
                                    1
                                })
                                .sum::<usize>()
                                <= limit + 2,
                            "limit {limit}, width {width}: {line}"
                        );
                    }
                } else if width == 160 {
                    ensure!(
                        rows.iter().any(|line| line.chars().count() > 100),
                        "unset did not retain full width: {screen}"
                    );
                }
            }
            let completed_prose = prose_rows(&terminal);
            // The saved rollout is reconstructed by the actual resume command.
            let saved_home = std::mem::replace(&mut terminal._codex_home, tempfile::tempdir()?);
            drop(terminal);
            let mut resumed = PtyCodex::start_cli(
                &cwd,
                saved_home,
                if fullscreen {
                    &["resume", "--last"]
                } else {
                    &["resume", "--last", "--no-alt-screen"]
                },
            )?;
            resumed.wait_for_startup()?;
            resize(&mut resumed, 160)?;
            resumed.wait_for_screen("PROSE_DONE")?;
            wait_for_prose(&mut resumed, Some(&completed_prose))?;
            let resumed_prose = prose_rows(&resumed);
            ensure!(
                resumed_prose == completed_prose,
                "resume changed prose width: before {completed_prose:?}, after {resumed_prose:?}"
            );
            resumed.ensure_running()?;
        }
    }
    Ok(())
}

fn prose_rows(terminal: &PtyCodex) -> Vec<String> {
    let screen = terminal.screen_contents();
    let mut body = false;
    let mut rows = Vec::new();
    for line in screen.lines() {
        if line.trim_start().starts_with("• PROSE") {
            body = true;
        }
        if body && line.contains("PROSE_DONE") {
            break;
        }
        if body && !line.trim().is_empty() {
            rows.push(line.to_owned());
        }
    }
    rows
}

fn wait_for_prose(terminal: &mut PtyCodex, expected_rows: Option<&[String]>) -> Result<()> {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        let screen = terminal.screen_contents();
        let rows = prose_rows(terminal);
        // Unicode line breaking may split a Latin/Hangul boundary. Rejoin display rows and
        // verify every source character in order, excluding only gutters and wrap whitespace.
        let prose: String = rows
            .iter()
            .flat_map(|row| row.chars())
            .filter(|ch| !ch.is_whitespace() && *ch != '•')
            .collect();
        if prose == "PROSE한글English".repeat(24)
            && expected_rows.is_none_or(|expected| rows == expected)
        {
            return Ok(());
        }
        ensure!(
            std::time::Instant::now() < deadline,
            "prose lost content or failed to restore wrapping: {screen}"
        );
        terminal.read_output(Duration::from_millis(100))?;
        terminal.ensure_running()?;
    }
}
