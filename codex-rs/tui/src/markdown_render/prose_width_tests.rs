use super::*;
use crate::terminal_hyperlinks::lines_with_sources_eq;

fn render(source: &str, width: usize, limit: Option<usize>) -> Vec<HyperlinkLine> {
    crate::markdown::render_markdown_agent_with_list_spacing(
        source,
        Some(width),
        Some(Path::new("/")),
        None,
        ListSpacing::Uniform,
        limit,
    )
}

#[test]
fn prose_limit_wraps_unicode_lists_quotes_and_links() {
    let words = "한글 English `inline code` ".repeat(20);
    let source = format!(
        "# {words}\n\n{words} https://example.com/{}\n\n- {words}\n    - {words}\n\n> {words}\n> > {words}",
        "path/".repeat(30)
    );
    let baseline = render(&source, 158, None);
    assert!(baseline.iter().any(|line| line.width() > 100));
    for limit in [40, 80, 100] {
        for width in [158, 58, 158] {
            let lines = render(&source, width, Some(limit));
            assert!(lines.iter().all(|line| line.width() <= width.min(limit)));
            let sources = |lines: &[HyperlinkLine]| {
                let mut result = Vec::new();
                for line in lines {
                    if let Some(source) = &line.source {
                        let entry = (source.text.to_string(), source.copy.clone());
                        if result.last() != Some(&entry) {
                            result.push(entry);
                        }
                    }
                }
                result
            };
            assert_eq!(sources(&lines), sources(&baseline));
        }
    }
}

#[test]
fn rich_blocks_and_unset_rendering_remain_unchanged() {
    let code = format!("```text\n{}\n```", "code ".repeat(28));
    let table = format!(
        "| Heading | Other |\n|---|---|\n| {} | {} |\n",
        "cell ".repeat(20),
        "other ".repeat(10)
    );
    let math = format!("$$\n{}\n$$", "x + ".repeat(30));
    let diagram = "```mermaid\nflowchart LR\n A[Start] --> B[Finish]\n```";
    for source in [code.as_str(), table.as_str(), math.as_str(), diagram] {
        for width in [158, 58] {
            assert!(
                lines_with_sources_eq(
                    &render(source, width, None),
                    &render(source, width, Some(40))
                ),
                "{source}"
            );
        }
    }
}
