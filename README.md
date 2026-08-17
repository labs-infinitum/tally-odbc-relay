# tally-odbc-relay

Windows console app that exposes TallyPrime ODBC as a localhost HTTP API on port **9001**.

Install the exe in a CrossOver bottle or on a Windows machine where Tally is running with ODBC enabled. Clients (including macOS) then send SQL to `http://127.0.0.1:9001`.

```
curl / SDK  -->  tally-odbc-relay.exe :9001  -->  TallyODBC64_9000  -->  tally.exe :9000
```

This is not a Tally XML SDK. It only relays ODBC `SELECT` (and other Tally SQL the driver accepts).

## Requirements

- TallyPrime running, company loaded
- ODBC server enabled (default port `9000`)
- DSN `TallyODBC64_9000` (Tally registers this)
- This exe running **inside the same Windows environment** as the Tally ODBC driver (`TallyWin.Dat` / `TallyWin.dll`)

Host `isql` / macOS `pyodbc` cannot load Tally’s Windows driver. The relay is the Windows-side process; HTTP is the cross-OS surface.

## HTTP API

Default bind: `127.0.0.1:9001` (localhost only).

### `GET /health`

Connects to the DSN. `200` if Tally ODBC answers:

```json
{"ok":true}
```

`503` if it does not:

```json
{"ok":false,"error":"..."}
```

### `POST /query`

JSON:

```bash
curl -sS http://127.0.0.1:9001/query \
  -H 'Content-Type: application/json' \
  -d '{"sql":"SELECT $Name, $Parent, $ClosingBalance FROM Ledger"}'
```

Raw SQL:

```bash
curl -sS http://127.0.0.1:9001/query \
  -H 'Content-Type: text/plain' \
  --data-binary 'SELECT $Name FROM Company'
```

Success:

```json
{
  "columns": ["$Name"],
  "rows": [["Acme Traders"], ["Demo Company Pvt. Ltd."]]
}
```

Errors: `{"error":"..."}` with `400` (bad body), `502` (ODBC/Tally), or `500`.

Tally SQL examples:

```sql
SELECT $Name FROM Company
SELECT $Name FROM ODBCTables
SELECT $Name, $Parent, $ClosingBalance FROM Ledger
SELECT TOP 15 $Name FROM Ledger
```

## Run on Windows

1. Enable ODBC in Tally: **F1 → Settings → Connectivity → Enable ODBC Server**, port `9000`.
2. Confirm DSN **TallyODBC64_9000** in *ODBC Data Sources (64-bit)*.
3. Download `tally-odbc-relay.exe` from a GitHub Release (or CI artifact) and run it.

```bat
tally-odbc-relay.exe
tally-odbc-relay.exe --port 9001 --dsn TallyODBC64_9000
```

Flags and environment variables:

| Flag | Env | Default |
|---|---|---|
| `--bind` | `TALLY_ODBC_BIND` | `127.0.0.1` |
| `--port` | `TALLY_ODBC_PORT` | `9001` |
| `--dsn` | `TALLY_ODBC_DSN` | `TallyODBC64_9000` |

## Fresh CrossOver Tally install (macOS)

Wine’s ODBC manager only `LoadLibrary`s a Windows driver whose path ends in **`.dll`**. A stock Tally install registers `TallyWin.Dat`, so ODBC clients inside CrossOver fail until you add a `.dll` copy and point the DSN at it. The relay does **not** do this automatically.

### 1. Install Tally in a CrossOver bottle

Create a bottle (for example `Tally Prime`), install TallyPrime, enable **ODBC Server** on port `9000`, and load a company.

### 2. Copy the driver to a `.dll` name

In the bottle (PowerShell / `cmd` / CrossOver wine):

```bat
copy "C:\Program Files\TallyPrime\TallyWin.Dat" "C:\Program Files\TallyPrime\TallyWin.dll"
```

Leave `TallyWin.Dat` in place. Tally still uses it; Wine needs the `.dll` suffix.

Tally may rewrite the ODBC driver registry back to `TallyWin.Dat` (for example after a restart or ODBC re-register). If `SQLDriverConnect` starts failing with return code `5` and empty diagnostics, re-run the `reg add` steps below.

If Tally lives under `TallyPrimeEL` or another folder, use that path instead.

### 3. Point the ODBC driver and DSN at the `.dll`

From the Mac host, using CrossOver’s `wine` (bottle name must match):

```bash
WINE="/Users/$USER/Applications/CrossOver.app/Contents/SharedSupport/CrossOver/bin/wine"
BOTTLE="Tally Prime"

"$WINE" --bottle "$BOTTLE" --no-gui --wait reg add \
  "HKLM\Software\ODBC\ODBCINST.INI\Tally ODBC Driver64" \
  /v Driver /t REG_SZ /d "C:\Program Files\TallyPrime\TallyWin.dll" /f

"$WINE" --bottle "$BOTTLE" --no-gui --wait reg add \
  "HKLM\Software\ODBC\ODBC.INI\TallyODBC64_9000" \
  /v driver /t REG_SZ /d "C:\Program Files\TallyPrime\TallyWin.dll" /f
```

Or inside the bottle:

```bat
reg add "HKLM\Software\ODBC\ODBCINST.INI\Tally ODBC Driver64" /v Driver /t REG_SZ /d "C:\Program Files\TallyPrime\TallyWin.dll" /f
reg add "HKLM\Software\ODBC\ODBC.INI\TallyODBC64_9000" /v driver /t REG_SZ /d "C:\Program Files\TallyPrime\TallyWin.dll" /f
```

### 4. Run the relay inside that bottle

Copy `tally-odbc-relay.exe` into the bottle (for example `C:\tmp\`) and start it with the same `wine` binary:

```bash
"$WINE" --bottle "$BOTTLE" --no-gui --wait \
  'C:\tmp\tally-odbc-relay.exe'
```

You should see:

```
tally-odbc-relay listening on http://127.0.0.1:9001 (DSN=TallyODBC64_9000)
```

Keep this process running. Port `9001` is published on the Mac loopback the same way Tally’s `9000` is.

### 5. Query from the Mac host

```bash
curl -sS http://127.0.0.1:9001/health

curl -sS http://127.0.0.1:9001/query \
  -H 'Content-Type: application/json' \
  -d '{"sql":"SELECT $Name FROM Company"}'
```

### CrossOver checklist

- [ ] Tally is running in the bottle and a company is loaded
- [ ] `Enable ODBC Server=Yes` and `ServerPort=9000` in `tally.ini`
- [ ] `TallyWin.dll` exists next to `TallyWin.Dat`
- [ ] Registry `Driver` / DSN `driver` values end in `TallyWin.dll` (not `TallyWin.Dat`)
- [ ] `tally-odbc-relay.exe` is running **in that bottle**, not as a native macOS process
- [ ] `curl http://127.0.0.1:9001/health` returns `{"ok":true}`

Do not use Wine’s ADO/`cscript` path (`DSN=(null)`). This exe calls `SQLDriverConnect` / `SQLExecDirect` directly.

## Build

The binary links Windows `odbc32`. Produce the exe on Windows or in CI.

```bat
cargo test --locked
cargo build --release --locked
```

Output: `target\release\tally-odbc-relay.exe`

On macOS you can run `cargo test --locked` for JSON/API unit tests. ODBC execution is Windows-only.

## CI / CD

- **CI** (`.github/workflows/ci.yml`): `windows-latest` fmt, clippy, test, release build; uploads the exe as an artifact.
- **Release** (`.github/workflows/release.yml`): push a `v*` tag (for example `v0.1.0` matching `Cargo.toml`) to attach `tally-odbc-relay.exe` to a GitHub Release.

## License

Apache License 2.0. See [LICENSE](LICENSE).
