dump-dependency
====

謎のソースコード依存関係抽出ツール

現在の機能：
- `list`: `compile_commands.json` にあるソースコードの依存先を一覧するサブコマンド


How to install
----
```shell
cargo install --git https://github.com/K-atc/dump-dependency.git --bins --all-features
```

Or manually git clone and:

```shell
cargo install --path . --bins --all-features
```


How to use
----
```shell
$ dump-dependency ./compile_commands.json --exclude-system-headers --headers list
cpp/poppler-destination-private.h
cpp/poppler-destination.h
cpp/poppler-document-private.h
cpp/poppler-document.h
cpp/poppler-embedded-file-private.h
cpp/poppler-embedded-file.h
cpp/poppler-font-private.h
cpp/poppler-font.h
cpp/poppler-global.h
cpp/poppler-image-private.h
cpp/poppler-image.h
[...]
```