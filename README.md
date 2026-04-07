# markdown-termshot

`markdown-termshot` 是一个用于处理 Markdown 命令的 Rust 命令行工具。

它会扫描 Markdown 中的 shell 代码块，找出其中**每个非空行都以 `$` 开头**的命令块，调用 `termshot` 执行命令并截图，然后把原来的代码块替换为 Markdown 图片引用。

> [!WARN]
> 代码由 GPT-5.4 完成。提示词见 [PROMPT.md](./PROMPT.md)。仅测试过简单案例和我自己的作业，未经充分测试和 Review。

## AI 时代的大学作业报告应该怎么做？

AI 生成代码 -> AI 生成测试 -> AI 生成作业报告 -> markdown-termshot 生成优质的命令执行截图放进报告中 -> pandoc 转 Word，可能需要手动调整格式

AI 生成作业报告这一步，应该指导 AI：

````
注意报告中，涉及要执行命令 -> 截图的部分，应该使用如下的写法（Bash 代码块 + dollar 符前置）：

```bash
$ some-command
```
````

## 功能目标

适用于这类 Markdown 内容：

````markdown
```bash
$ ls -1 | grep rs
$ pwd
```
````

工具会：

- 提取 `$` 后面的命令内容
- 使用 `termshot` 运行并截图
- 默认将图片保存到**输入 Markdown 文件所在目录**下的 `public/`（可通过 `--pic-dir` 覆盖）
- 把原命令块替换为图片引用，或在保留代码块时追加图片

## 路径与运行目录规则

- `termshot` 执行命令时，**工作目录始终是输入 Markdown 文件所在目录**
- `--pic-dir` 如果是相对路径，也会相对于**输入 Markdown 文件所在目录**解析
- 例如输入文件是 `docs/report.md`，未指定 `--pic-dir` 时，图片会输出到 `docs/public/`
- `--output` 只影响改写后的 Markdown 写到哪里，不影响命令运行目录
- 当 `--output` 指向其他目录时，图片引用路径会自动按**输出 Markdown 文件的位置**计算相对路径

## 匹配规则

当前只处理 fenced code block，且语言标记为以下之一：

- `bash`
- `sh`
- `shell`
- `zsh`

同时要求代码块内容满足：

- 每个**非空行**都必须以 `$` 开头
- 空行允许存在
- 如果代码块中混入普通输出行，则不会处理

例如下面这个代码块会被处理：

````markdown
```bash
$ echo hello
$ printf 'done\n'
```
````

而下面这个不会被处理：

````markdown
```bash
$ echo hello
hello
```
````

## 依赖

本工具依赖外部命令 `termshot`。

请先确保本机可以直接运行：

```bash
termshot --show-cmd --filename "test.png" -- "echo hello"
```

`termshot` 项目地址：
[`homeport/termshot`](https://github.com/homeport/termshot)

## 构建

```bash
cargo build --release
```

## 基本用法

### 原地修改 Markdown

```bash
cargo run -- path/to/doc.md
```

这会：

- 读取 `path/to/doc.md`
- 在 `path/to/` 下创建默认图片目录 `public/`
- 在 `path/to/` 作为工作目录运行命令并截图
- 直接改写原 Markdown 文件

### 指定图片目录

```bash
cargo run -- --pic-dir assets/termshots path/to/doc.md
```

如果这里的 `assets/termshots` 是相对路径，那么它实际会被解析为 `path/to/assets/termshots`。

### 保留原代码块

```bash
cargo run -- --keep-code-block path/to/doc.md
```

开启后，输出效果类似：

````markdown
```bash
$ ls -1 | grep rs
$ pwd
```

![termshot](public/doc-001.png)
````

### 输出到新文件

```bash
cargo run -- --output path/to/doc.rendered.md path/to/doc.md
```

开启后：

- 原文件 `path/to/doc.md` 不变
- 改写后的内容写入 `path/to/doc.rendered.md`
- 命令仍然在输入文件所在目录执行
- 图片仍默认生成在输入文件所在目录下的 `public/`

### 组合使用

```bash
cargo run -- \
  --pic-dir public \
  --keep-code-block \
  --output docs/guide.rendered.md \
  docs/guide.md
```

## 命令行参数

- `--pic-dir <DIR>`：图片输出目录，默认是 `public`；相对路径按输入 Markdown 所在目录解析
- `--keep-code-block`：保留原始命令代码块，并在后面追加图片
- `--output <FILE>`：将改写结果输出到新 Markdown 文件，而不是原地覆盖输入文件
- `<INPUT>`：输入的 Markdown 文件路径

## 文件命名

生成的图片文件名会基于输入 Markdown 文件名自动编号，例如：

- `guide-001.png`
- `guide-002.png`

## 当前行为说明

- 如果没有匹配到可处理的命令块，程序不会修改任何文件
- 如果 `termshot` 执行失败，程序会直接返回错误
- 当前不会处理缩进式代码块，只处理 fenced code block
- 当前不会保留命令执行输出，只会对命令本身截图
