use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};
use clap::Parser;
use pathdiff::diff_paths;

#[derive(Debug, Parser)]
#[command(
    author,
    version,
    about = "Render $-prefixed shell code blocks in Markdown with termshot"
)]
struct Cli {
    #[arg(long, default_value = "public")]
    pic_dir: PathBuf,

    #[arg(long)]
    keep_code_block: bool,

    #[arg(long)]
    output: Option<PathBuf>,

    input: PathBuf,
}

#[derive(Debug, Clone)]
struct CommandBlock {
    range: std::ops::Range<usize>,
    command: String,
}

#[derive(Debug, Clone)]
struct Fence {
    marker: char,
    len: usize,
    info: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    run(cli)
}

fn run(cli: Cli) -> Result<()> {
    let input_path = cli
        .input
        .canonicalize()
        .with_context(|| format!("failed to resolve input file {}", cli.input.display()))?;
    let input_dir = input_path.parent().ok_or_else(|| {
        anyhow!(
            "failed to find parent directory for input file {}",
            input_path.display()
        )
    })?;
    let output_path = resolve_output_path(cli.output.as_deref(), &input_path)?;

    let markdown = fs::read_to_string(&input_path)
        .with_context(|| format!("failed to read {}", input_path.display()))?;

    let blocks = find_command_blocks(&markdown);
    if blocks.is_empty() {
        return Ok(());
    }

    let pic_dir = resolve_pic_dir(&cli.pic_dir, input_dir);
    fs::create_dir_all(&pic_dir)
        .with_context(|| format!("failed to create picture directory {}", pic_dir.display()))?;

    let output_markdown_dir = output_path.parent().ok_or_else(|| {
        anyhow!(
            "failed to find parent directory for {}",
            output_path.display()
        )
    })?;

    let replacements = blocks
        .iter()
        .enumerate()
        .map(|(index, block)| {
            let filename = make_image_filename(&input_path, index + 1);
            let image_path = pic_dir.join(&filename);
            run_termshot(input_dir, &image_path, &block.command)?;

            let image_markdown = make_image_markdown(&image_path, output_markdown_dir);

            let replacement = if cli.keep_code_block {
                let original_block = markdown[block.range.clone()].trim_end_matches(['\r', '\n']);
                format!("{original_block}\n\n{image_markdown}")
            } else {
                image_markdown
            };

            let replacement = if markdown[block.range.clone()].ends_with('\n') {
                format!("{replacement}\n")
            } else {
                replacement
            };

            Ok::<_, anyhow::Error>(replacement)
        })
        .collect::<Result<Vec<_>>>()?;

    let rewritten = rewrite_markdown(&markdown, &blocks, &replacements);

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create output directory {}", parent.display()))?;
    }

    fs::write(&output_path, rewritten)
        .with_context(|| format!("failed to write {}", output_path.display()))?;

    Ok(())
}

fn resolve_pic_dir(pic_dir: &Path, input_dir: &Path) -> PathBuf {
    if pic_dir.is_absolute() {
        pic_dir.to_path_buf()
    } else {
        input_dir.join(pic_dir)
    }
}

fn resolve_output_path(output: Option<&Path>, input_path: &Path) -> Result<PathBuf> {
    match output {
        Some(path) if path.is_absolute() => Ok(path.to_path_buf()),
        Some(path) => Ok(env::current_dir()
            .context("failed to get current working directory")?
            .join(path)),
        None => Ok(input_path.to_path_buf()),
    }
}

fn run_termshot(working_dir: &Path, image_path: &Path, command: &str) -> Result<()> {
    let status = Command::new("termshot")
        .current_dir(working_dir)
        .arg("--show-cmd")
        .arg("--filename")
        .arg(image_path)
        .arg("--")
        .arg(command)
        .status()
        .context("failed to spawn termshot")?;

    if !status.success() {
        bail!("termshot exited with status {status}");
    }

    Ok(())
}

fn rewrite_markdown(markdown: &str, blocks: &[CommandBlock], replacements: &[String]) -> String {
    let mut result = String::with_capacity(markdown.len());
    let mut cursor = 0;

    for (block, replacement) in blocks.iter().zip(replacements) {
        result.push_str(&markdown[cursor..block.range.start]);
        result.push_str(replacement);
        cursor = block.range.end;
    }

    result.push_str(&markdown[cursor..]);
    result
}

fn find_command_blocks(markdown: &str) -> Vec<CommandBlock> {
    let mut blocks = Vec::new();
    let mut cursor = 0;

    while cursor < markdown.len() {
        let (line, next_cursor) = next_line(markdown, cursor);

        if let Some(fence) = parse_opening_fence(line)
            && let Some((end, content)) = find_fence_end(markdown, next_cursor, &fence)
        {
            if is_shell_language(&fence.info)
                && let Some(command) = parse_prompted_command(&content)
            {
                blocks.push(CommandBlock {
                    range: cursor..end,
                    command,
                });
            }

            cursor = end;
            continue;
        }

        cursor = next_cursor;
    }

    blocks
}

fn find_fence_end(markdown: &str, start: usize, fence: &Fence) -> Option<(usize, String)> {
    let mut cursor = start;
    let mut content = String::new();

    while cursor < markdown.len() {
        let (line, next_cursor) = next_line(markdown, cursor);
        if is_closing_fence(line, fence) {
            return Some((next_cursor, content));
        }

        content.push_str(line);
        cursor = next_cursor;
    }

    None
}

fn next_line(text: &str, start: usize) -> (&str, usize) {
    if start >= text.len() {
        return ("", text.len());
    }

    match text[start..].find('\n') {
        Some(relative_end) => {
            let end = start + relative_end + 1;
            (&text[start..end], end)
        }
        None => (&text[start..], text.len()),
    }
}

fn parse_opening_fence(line: &str) -> Option<Fence> {
    let line = line.trim_end_matches(['\r', '\n']);
    let indent = line.chars().take_while(|ch| *ch == ' ').count();
    if indent > 3 {
        return None;
    }

    let rest = &line[indent..];
    let marker = rest.chars().next()?;
    if marker != '`' && marker != '~' {
        return None;
    }

    let len = rest.chars().take_while(|ch| *ch == marker).count();
    if len < 3 {
        return None;
    }

    let info = rest[len..].trim().to_string();
    if marker == '`' && info.contains('`') {
        return None;
    }

    Some(Fence { marker, len, info })
}

fn is_closing_fence(line: &str, fence: &Fence) -> bool {
    let line = line.trim_end_matches(['\r', '\n']);
    let indent = line.chars().take_while(|ch| *ch == ' ').count();
    if indent > 3 {
        return false;
    }

    let rest = &line[indent..];
    let len = rest.chars().take_while(|ch| *ch == fence.marker).count();
    if len < fence.len {
        return false;
    }

    rest[len..].trim().is_empty()
}

fn is_shell_language(info: &str) -> bool {
    matches!(
        info.split_whitespace()
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "bash" | "sh" | "shell" | "zsh"
    )
}

fn parse_prompted_command(content: &str) -> Option<String> {
    let mut lines = Vec::new();
    let mut has_command = false;

    for line in content.lines() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            lines.push(String::new());
            continue;
        }

        let command = line.strip_prefix('$')?;
        let command = command.strip_prefix(' ').unwrap_or(command);
        lines.push(command.to_string());
        has_command = true;
    }

    has_command.then(|| lines.join("\n"))
}

fn make_image_filename(input_path: &Path, index: usize) -> String {
    let stem = input_path
        .file_stem()
        .and_then(|value| value.to_str())
        .map(sanitize_filename)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "termshot".to_string());

    format!("{stem}-{index:03}.png")
}

fn sanitize_filename(value: &str) -> String {
    let mut sanitized = String::new();
    let mut previous_dash = false;

    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            sanitized.push(ch.to_ascii_lowercase());
            previous_dash = false;
        } else if !previous_dash {
            sanitized.push('-');
            previous_dash = true;
        }
    }

    sanitized.trim_matches('-').to_string()
}

fn make_image_markdown(image_path: &Path, markdown_dir: &Path) -> String {
    let relative_image_path =
        diff_paths(image_path, markdown_dir).unwrap_or_else(|| image_path.to_path_buf());
    format!("![termshot]({})", to_markdown_path(&relative_image_path))
}

fn to_markdown_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_prompted_commands() {
        let command = parse_prompted_command("$ echo hello\n$ printf 'done\\n'\n").unwrap();
        assert_eq!(command, "echo hello\nprintf 'done\\n'");
    }

    #[test]
    fn rejects_code_blocks_with_output_lines() {
        assert!(parse_prompted_command("$ echo hello\nhello\n").is_none());
    }

    #[test]
    fn finds_only_shell_command_fences() {
        let markdown = concat!(
            "before\n\n",
            "```bash\n",
            "$ echo hello\n",
            "```\n\n",
            "```text\n",
            "$ echo no\n",
            "```\n",
        );

        let blocks = find_command_blocks(markdown);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].command, "echo hello");
    }

    #[test]
    fn rewrites_multiple_blocks() {
        let markdown = concat!(
            "A\n\n",
            "```bash\n",
            "$ echo one\n",
            "```\n\n",
            "B\n\n",
            "```bash\n",
            "$ echo two\n",
            "```\n",
        );

        let blocks = find_command_blocks(markdown);
        let replacements = vec![
            "![termshot](public/a.png)\n".to_string(),
            "![termshot](public/b.png)\n".to_string(),
        ];

        let rewritten = rewrite_markdown(markdown, &blocks, &replacements);
        assert_eq!(
            rewritten,
            concat!(
                "A\n\n",
                "![termshot](public/a.png)",
                "\n\nB\n\n",
                "![termshot](public/b.png)",
                "\n",
            )
        );
    }

    #[test]
    fn builds_image_markdown_relative_to_output_file() {
        let image_path = Path::new("/tmp/project/public/demo-001.png");
        let markdown_dir = Path::new("/tmp/project/docs/guides");

        let image_markdown = make_image_markdown(image_path, markdown_dir);
        assert_eq!(image_markdown, "![termshot](../../public/demo-001.png)");
    }

    #[test]
    fn resolves_relative_pic_dir_from_input_markdown_directory() {
        let pic_dir = resolve_pic_dir(Path::new("public"), Path::new("/tmp/project/docs"));
        assert_eq!(pic_dir, PathBuf::from("/tmp/project/docs/public"));
    }
}
