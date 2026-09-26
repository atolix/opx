# Deploy

## Build

- [x] 依存関係をインストールする

```sh
cargo fetch
```

- [x] アプリケーションをビルドする

```sh
cargo build --release
```

- [x] テストを実行する

```sh
cargo test
```

## Database

- [x] データベースのバックアップを確認する

- [x] migrationを実行する

```sh
./bin/migrate production
```

## Production

- [ ] 新しいバージョンをdeployする

```sh
./bin/deploy production
```

### Verification

- [ ] rolloutの状態を確認する

```sh
kubectl rollout status deployment/app -n production
```

- [ ] health checkを確認する

```sh
curl --fail https://example.com/health
```

- [ ] 問題がないことを確認して完了報告する
