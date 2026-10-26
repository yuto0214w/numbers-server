# numbers-server

チェッカーと独自の要素を組み合わせたオリジナルのボードゲームです。

## 実行

コードの実行・ビルドには [Rust](https://www.rust-lang.org/) のインストールが必要です。
また，ドキュメント内の図の描画には [PlantUML](https://plantuml.com/) のインストールが必要です。(任意)

```
cargo r
```

## 設定

実行ファイルと同じディレクトリにある `config.toml` を読みます。

なければ [`res/config.toml`](res/config.toml) と同じ内容で作成されます。

## API ドキュメント

(設定でオフにできます)

サーバーの起動中に `/` (設定を変えていなければ [`http://localhost:8080/`](http://localhost:8080/)) にアクセスすることで確認できます。

OpenAPI の定義そのものは

- `/openapi.json`
- `/openapi.yaml`

で配信されます。

## 感謝

- @kagesakura
  - コードに関する助言やゲームデザインの相談等
