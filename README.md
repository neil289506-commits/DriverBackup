# DriverBackup

DriverBackup is a Windows desktop utility for exporting and importing third-party device drivers. It uses the built-in `pnputil.exe` tool to back up the installed driver store and restore drivers from saved `.inf` files.

The app is written in Rust and provides a simple graphical interface built with egui.

## Features

- Export all third-party drivers from the current Windows system to a selected folder.
- Automatically generate a `sys.session` export log containing:
  - Windows version
  - export timestamp
  - total driver count
  - full driver list
- Import a single `.inf` driver or an entire folder of drivers.
- Detect duplicate or conflicting installed drivers before import.
- Optionally overwrite previously installed drivers when requested by the user.
- Uses the Windows Driver Store via `pnputil.exe`.

## Requirements

- Windows 10 or Windows 11
- Run the program as Administrator
- The `pnputil.exe` utility must be available through the system PATH (it is included with Windows)

Important: exporting and importing drivers requires administrator privileges. If the program is launched without elevated permissions, the operation may fail.

## How it works

DriverBackup does the following:

1. Scans the current Windows driver store.
2. Exports all third-party drivers to a user-selected directory.
3. Saves a `sys.session` record file in the export folder.
4. On import, scans the target `.inf` files and checks whether an equivalent driver is already installed.
5. If a duplicate is found, the user can choose to overwrite the existing driver or skip it.

## Screenshots

The app includes a dark themed GUI with two tabs:

- Export Drivers
- Import Drivers

## Build

Requirements:

- Rust toolchain
- Cargo
- Windows build target

Build the project with:

```bash
cargo build --release
```

Then run the generated executable from the target output directory.

## Run

After building, launch the compiled binary:

```bash
./target/release/driver_backup.exe
```

## Usage

### Export drivers

1. Open the app as Administrator.
2. Select the destination folder for the backup.
3. Click `Start Export`.
4. DriverBackup will export all third-party drivers to that folder and create `sys.session`.

### Import drivers

1. Open the app as Administrator.
2. Select either:
   - a single `.inf` file, or
   - a folder containing multiple `.inf` files
3. Click `Start Import`.
4. If a driver already exists, the app will ask whether to overwrite it.

## Project structure

```text
.
├── assets/
│   └── msjh.ttf
├── src/
│   └── main.rs
├── build.rs
├── Cargo.toml
├── License
├── README.md
└── .gitignore
```

## License

This project is licensed under the terms of the repository's `License` file.

## Notes

This tool interacts directly with the Windows driver store and is intended for advanced users and IT administrators. Use with caution when managing system drivers.
