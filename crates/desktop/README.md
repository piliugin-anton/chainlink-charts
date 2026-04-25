# chainlink-charts-desktop

Нативный клиент (Rust, **egui** / **eframe**) к существующему Next.js BFF в этом репозитории. Секреты Chainlink остаются только на сервере; десктоп ходит по HTTP к `GET /api/chainlink/history` и `GET /api/chainlink/stream`.

## Требования

- Rust toolchain (stable).
- Запущенный Next.js с настроенными переменными Chainlink (`next dev` или `next start`).

## Запуск

1. В корне репозитория (или из этой папки) соберите и запустите:

   ```bash
   cargo run -p chainlink-charts-desktop
   ```

2. Базовый URL BFF по умолчанию: `http://127.0.0.1:3000`. Чтобы указать другой хост:

   ```bash
   CHAINLINK_CHARTS_BASE_URL=https://example.com cargo run -p chainlink-charts-desktop
   ```

3. Убедитесь, что Next.js слушает тот же origin (порт и схема совпадают с `CHAINLINK_CHARTS_BASE_URL`).

## Поведение

- **Список активов** — те же пары, что в `src/lib/chainlink/constants.ts` (BTC/ETH/SOL/XRP).
- **Экран актива** — история за **последние 24 часа**, интервал **5m** или **15m**, **свечи OHLC** (**egui_plot::BoxPlot**, как в обсуждении [egui#967](https://github.com/emilk/egui/issues/967)). Поля `open`/`high`/`low`/`close` в JSON и тик `p` в стриме — в масштабе **1e18** (как в `src/lib/chainlink/price.ts`), перед отрисовкой делятся на `1e18`.
- **Назад** — возврат к списку.

## Тесты

```bash
cargo test -p chainlink-charts-desktop
```
