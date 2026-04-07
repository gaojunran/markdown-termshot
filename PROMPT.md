帮我实现这个 Rust 命令行工具。我想做的是从 Markdown 中抽出形如

```bash
$ some-command
```

（必须以 dollar 开头），并用 termshot 运行这个命令并截图，将图片文件存在 public/ 下（这个文件夹可以用 --pic-dir 覆盖），同时把原有的多行命令块替换成图片引用 Markdown 语法（可以用 --keep-code-block 来保留原有的多行命令块）

ref：https://github.com/homeport/termshot

使用方法：termshot --show-cmd --filename "xxx.png" -- "ls -1 | grep go"

你应该尽量使用已有的库来实现。Rust 代码风格应该尽量 Idiomatic。

====

- **`--output`**：输出到新 Markdown 文件而不是原地覆盖

这个先帮我实现一下，然后写个中文的 README。README 不要太花哨，聚焦这个 CLI 要实现的目标以及基本用法即可。

====

命令的运行目录应该始终是输入 markdown 文件所在目录，public/ 也要建在这个目录下。我看你现在似乎是在 public/ 下运行？这是不对的

