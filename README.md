# opx

Markdownを正本として扱う、ターミナル向けのrunbook/checklist CLIです。チェック状態はMarkdownのtask marker (`[ ]` / `[x]`) に直接保存します。独自DBや状態ファイルは作成しません。

## インストール

Rust stableを用意して、リポジトリのルートで実行します。

```sh
cargo install --path .
```

## Markdown例

````md
# Deploy

## Build

- [ ] Docker imageをbuildする

```sh
docker build -t app .
```

- [ ] imageを確認する
````

taskの直後（空行は可）にあるfenced code blockが、そのtaskの関連commandになります。commandは自動実行されません。

## CLI

```sh
opx tui runbook.md
opx status runbook.md
opx status runbook.md --json
opx next runbook.md --json
opx check runbook.md 0
opx uncheck runbook.md 0
opx copy runbook.md 0
```

task indexは0始まりです。通常の結果はstdout、エラーはstderrに出力されます。未完了taskがない場合の `next` は非0終了します。

JSONの例:

```json
{
  "index": 1,
  "section": "Database",
  "title": "migrationを実行する",
  "checked": false,
  "language": "sh",
  "command": "bundle exec rails db:migrate"
}
```

`status --json` は `total`、`completed`、`progress`、`tasks` を返します。多層headingの場合、taskには最も近いheadingが `section` として入り、上位からの階層が `section_path` に入ります。

## TUIキーバインド

| キー | 操作 |
| --- | --- |
| `j` / `Down` | 次のtask |
| `k` / `Up` | 前のtask |
| `Space` / `x` | checked / unchecked切り替え（即時保存） |
| `y` | 関連commandをclipboardへコピー |
| `Enter` | 右側のdetail panelを開閉 |
| `n` | 次の未完了taskへ移動 |
| `q` | 終了 |

コードブロックの内容を実行する機能はありません。コピー操作もclipboardへの書き込みだけを行います。

TUIでは、上部に完了数とプログレスバーを表示します。選択中taskは黄色、完了taskは水色、未完了taskは灰色で表示されます。`y` でコピーすると画面下部にコピー結果が表示されます。`Enter` で開く右側panelにはtaskの状態、階層、language、commandを表示します。
