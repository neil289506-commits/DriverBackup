// 驅動程式備份 / 還原工具
// 需求：Windows 10/11，且必須以「系統管理員」身分執行（pnputil 匯出/匯入驅動需要 Admin 權限）
//
// 功能：
//   1. 匯出電腦上所有第三方驅動程式到指定資料夾，並自動產生 sys.session 紀錄檔
//   2. 匯入單一驅動 (.inf) 或整個資料夾內的所有驅動
//   3. 匯入時若偵測到相同驅動已安裝，會詢問是否覆蓋
//
// 核心技術：呼叫 Windows 內建的 pnputil.exe 來操作驅動存放區(Driver Store)

#![windows_subsystem = "windows"] // 發行版不彈出主控台黑窗

use eframe::egui;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

// ---------------------------------------------------------------------
// 資料結構
// ---------------------------------------------------------------------

#[derive(Debug, Clone)]
struct DriverEntry {
    /// 資料夾內的 inf 檔名，例如 nvhda.inf
    inf_name: String,
    /// 該驅動所在的子資料夾路徑
    folder: PathBuf,
}

#[derive(Debug, Clone)]
struct InstalledDriver {
    /// pnputil 內部發佈名稱，例如 oem12.inf
    published_name: String,
    /// 原始 inf 名稱，例如 nvhda.inf
    original_name: String,
}

/// 背景執行緒 -> UI 的訊息
enum WorkerMsg {
    Log(String),
    /// 匯入前發現重複驅動，需要使用者確認是否覆蓋
    NeedConfirm(Vec<(PathBuf, InstalledDriver)>),
    Done,
}

#[derive(PartialEq, Clone, Copy)]
enum Tab {
    Export,
    Import,
}

enum ImportStage {
    Idle,
    /// 等待使用者確認是否覆蓋（重複清單，來源檔路徑）
    Confirming(Vec<(PathBuf, InstalledDriver)>),
    Running,
}

// ---------------------------------------------------------------------
// App
// ---------------------------------------------------------------------

struct DriverApp {
    tab: Tab,

    // 匯出
    export_path: Option<PathBuf>,
    export_running: bool,

    // 匯入
    import_target: Option<PathBuf>,
    import_is_folder: bool,
    import_stage: ImportStage,

    // 共用
    log_lines: Vec<String>,
    rx: Option<Receiver<WorkerMsg>>,
    tx_to_worker: Option<Sender<bool>>, // 使用者的覆蓋選擇 (true=覆蓋)
}

impl Default for DriverApp {
    fn default() -> Self {
        Self {
            tab: Tab::Export,
            export_path: None,
            export_running: false,
            import_target: None,
            import_is_folder: false,
            import_stage: ImportStage::Idle,
            log_lines: vec!["就緒。請選擇左上方分頁開始操作。".to_string()],
            rx: None,
            tx_to_worker: None,
        }
    }
}

impl DriverApp {
    fn push_log(&mut self, s: impl Into<String>) {
        self.log_lines.push(s.into());
        if self.log_lines.len() > 2000 {
            self.log_lines.drain(0..500);
        }
    }

    fn poll_worker(&mut self) {
        let mut done = false;
        let mut need_confirm: Option<Vec<(PathBuf, InstalledDriver)>> = None;
        let mut logs = vec![];

        if let Some(rx) = &self.rx {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    WorkerMsg::Log(s) => logs.push(s),
                    WorkerMsg::NeedConfirm(v) => need_confirm = Some(v),
                    WorkerMsg::Done => done = true,
                }
            }
        }
        for l in logs {
            self.push_log(l);
        }
        if let Some(v) = need_confirm {
            self.import_stage = ImportStage::Confirming(v);
        }
        if done {
            self.export_running = false;
            if matches!(self.import_stage, ImportStage::Running) {
                self.import_stage = ImportStage::Idle;
            }
            self.rx = None;
        }
    }

    // -------------------------------------------------------------
    // 匯出
    // -------------------------------------------------------------
    fn start_export(&mut self) {
        let Some(dest) = self.export_path.clone() else {
            self.push_log("⚠ 請先選擇匯出資料夾。");
            return;
        };
        self.export_running = true;
        self.push_log(format!("開始匯出所有驅動程式到：{}", dest.display()));

        let (tx, rx) = channel();
        self.rx = Some(rx);

        thread::spawn(move || {
            let _ = std::fs::create_dir_all(&dest);
            let dest_str = dest.to_string_lossy().to_string();

            let output = run_pnputil(&["/export-driver", "*", &dest_str]);

            match output {
                Ok(out) => {
                    let text = String::from_utf8_lossy(&out.stdout).to_string();
                    for line in text.lines() {
                        if !line.trim().is_empty() {
                            let _ = tx.send(WorkerMsg::Log(line.to_string()));
                        }
                    }
                    if !out.status.success() {
                        let err = String::from_utf8_lossy(&out.stderr).to_string();
                        let _ = tx.send(WorkerMsg::Log(format!(
                            "⚠ pnputil 回傳非成功狀態：{}",
                            err
                        )));
                    }
                }
                Err(e) => {
                    let _ = tx.send(WorkerMsg::Log(format!(
                        "❌ 執行 pnputil 失敗：{e}（請確認以系統管理員身分執行本程式）"
                    )));
                    let _ = tx.send(WorkerMsg::Done);
                    return;
                }
            }

            // 掃描匯出資料夾，統計實際匯出的驅動
            let drivers = scan_exported_drivers(&dest);
            let _ = tx.send(WorkerMsg::Log(format!(
                "共匯出 {} 組驅動程式。",
                drivers.len()
            )));

            match write_session_file(&dest, &drivers) {
                Ok(_) => {
                    let _ = tx.send(WorkerMsg::Log("已建立 sys.session 紀錄檔。".to_string()));
                }
                Err(e) => {
                    let _ = tx.send(WorkerMsg::Log(format!("⚠ 建立 sys.session 失敗：{e}")));
                }
            }

            let _ = tx.send(WorkerMsg::Log("✅ 匯出完成。".to_string()));
            let _ = tx.send(WorkerMsg::Done);
        });
    }

    // -------------------------------------------------------------
    // 匯入
    // -------------------------------------------------------------
    fn start_import_scan(&mut self) {
        let Some(target) = self.import_target.clone() else {
            self.push_log("⚠ 請先選擇要匯入的驅動檔案或資料夾。");
            return;
        };
        self.push_log(format!("正在掃描：{}", target.display()));

        let (tx, rx) = channel();
        self.rx = Some(rx);
        self.import_stage = ImportStage::Running;

        thread::spawn(move || {
            let infs = collect_inf_files(&target);
            if infs.is_empty() {
                let _ = tx.send(WorkerMsg::Log("⚠ 找不到任何 .inf 驅動檔案。".to_string()));
                let _ = tx.send(WorkerMsg::Done);
                return;
            }
            let _ = tx.send(WorkerMsg::Log(format!("找到 {} 個 .inf 檔案，比對是否已安裝相同驅動…", infs.len())));

            let installed = enum_installed_drivers();
            let mut duplicates = vec![];
            for inf_path in &infs {
                if let Some(name) = inf_path.file_name().and_then(|n| n.to_str()) {
                    if let Some(found) = installed.iter().find(|d| d.original_name.eq_ignore_ascii_case(name)) {
                        duplicates.push((inf_path.clone(), found.clone()));
                    }
                }
            }

            if !duplicates.is_empty() {
                let _ = tx.send(WorkerMsg::Log(format!(
                    "偵測到 {} 個驅動與目前已安裝的版本相同/衝突，等待使用者確認…",
                    duplicates.len()
                )));
                let _ = tx.send(WorkerMsg::NeedConfirm(duplicates));
                // 這裡先不送 Done，等待使用者回應後由 confirm_import_choice 繼續處理
                return;
            }

            do_import(&infs, &[], &tx);
            let _ = tx.send(WorkerMsg::Done);
        });
    }

    /// 使用者在覆蓋確認對話框做出選擇後呼叫
    fn confirm_import_choice(&mut self, overwrite: bool, dups: Vec<(PathBuf, InstalledDriver)>, all_infs: Vec<PathBuf>) {
        self.import_stage = ImportStage::Running;
        let (tx, rx) = channel();
        self.rx = Some(rx);

        thread::spawn(move || {
            let overwrite_list: Vec<InstalledDriver> = if overwrite {
                dups.into_iter().map(|(_, d)| d).collect()
            } else {
                vec![]
            };
            do_import(&all_infs, &overwrite_list, &tx);
            let _ = tx.send(WorkerMsg::Done);
        });
    }
}

// ---------------------------------------------------------------------
// pnputil 呼叫輔助
// ---------------------------------------------------------------------

fn run_pnputil(args: &[&str]) -> std::io::Result<std::process::Output> {
    let mut cmd = Command::new("pnputil.exe");
    cmd.args(args);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd.output()
}

/// 掃描匯出資料夾，找出每個子資料夾內的 .inf 檔
fn scan_exported_drivers(dest: &Path) -> Vec<DriverEntry> {
    let mut result = vec![];
    if let Ok(entries) = std::fs::read_dir(dest) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Ok(inner) = std::fs::read_dir(&path) {
                    for f in inner.flatten() {
                        let fp = f.path();
                        if fp.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case("inf")) == Some(true) {
                            result.push(DriverEntry {
                                inf_name: fp.file_name().unwrap().to_string_lossy().to_string(),
                                folder: path.clone(),
                            });
                        }
                    }
                }
            }
        }
    }
    result
}

/// 遞迴收集資料夾內所有 .inf；若傳入的是單一檔案，直接回傳該檔案
fn collect_inf_files(target: &Path) -> Vec<PathBuf> {
    let mut result = vec![];
    if target.is_file() {
        if target.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case("inf")) == Some(true) {
            result.push(target.to_path_buf());
        }
        return result;
    }
    for entry in walkdir::WalkDir::new(target).into_iter().flatten() {
        let p = entry.path();
        if p.is_file() && p.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case("inf")) == Some(true) {
            result.push(p.to_path_buf());
        }
    }
    result
}

/// 解析 `pnputil /enum-drivers` 輸出，取得目前已安裝的第三方驅動清單
/// 相容繁中/英文兩種語系輸出
fn enum_installed_drivers() -> Vec<InstalledDriver> {
    let mut result = vec![];
    if let Ok(out) = run_pnputil(&["/enum-drivers"]) {
        let text = String::from_utf8_lossy(&out.stdout);
        let mut cur_pub: Option<String> = None;
        for raw_line in text.lines() {
            let line = raw_line.trim();
            let Some(idx) = line.find(':').or_else(|| line.find('：')) else { continue };
            let key = line[..idx].trim();
            let val = line[idx + 1..].trim().to_string();

            if key.contains("Published Name") || key.contains("已發佈的名稱") || key.contains("發佈名稱") {
                cur_pub = Some(val);
            } else if key.contains("Original Name") || key.contains("原始名稱") {
                if let Some(p) = cur_pub.take() {
                    result.push(InstalledDriver {
                        published_name: p,
                        original_name: val,
                    });
                }
            }
        }
    }
    result
}

/// 實際執行匯入：對每個 inf，若在 overwrite_list 內找到對應項目，先刪除舊驅動再安裝新驅動
fn do_import(infs: &[PathBuf], overwrite_list: &[InstalledDriver], tx: &Sender<WorkerMsg>) {
    for inf in infs {
        let name = inf.file_name().and_then(|n| n.to_str()).unwrap_or_default();

        if let Some(old) = overwrite_list.iter().find(|d| d.original_name.eq_ignore_ascii_case(name)) {
            let _ = tx.send(WorkerMsg::Log(format!("刪除舊驅動 {}（原始檔：{}）…", old.published_name, old.original_name)));
            let del = run_pnputil(&["/delete-driver", &old.published_name, "/uninstall", "/force"]);
            match del {
                Ok(out) => {
                    let t = String::from_utf8_lossy(&out.stdout);
                    for l in t.lines().filter(|l| !l.trim().is_empty()) {
                        let _ = tx.send(WorkerMsg::Log(format!("  {l}")));
                    }
                }
                Err(e) => {
                    let _ = tx.send(WorkerMsg::Log(format!("⚠ 刪除舊驅動失敗：{e}")));
                }
            }
        }

        let inf_str = inf.to_string_lossy().to_string();
        let _ = tx.send(WorkerMsg::Log(format!("安裝驅動：{}", inf_str)));
        match run_pnputil(&["/add-driver", &inf_str, "/install"]) {
            Ok(out) => {
                let t = String::from_utf8_lossy(&out.stdout);
                for l in t.lines().filter(|l| !l.trim().is_empty()) {
                    let _ = tx.send(WorkerMsg::Log(format!("  {l}")));
                }
                if !out.status.success() {
                    let e = String::from_utf8_lossy(&out.stderr);
                    let _ = tx.send(WorkerMsg::Log(format!("⚠ 安裝可能失敗：{e}")));
                }
            }
            Err(e) => {
                let _ = tx.send(WorkerMsg::Log(format!("❌ 安裝失敗：{e}")));
            }
        }
    }
    let _ = tx.send(WorkerMsg::Log("✅ 匯入流程結束。".to_string()));
}

// ---------------------------------------------------------------------
// Windows 版本資訊 (相當於 winver)
// ---------------------------------------------------------------------

#[cfg(windows)]
fn get_windows_version() -> String {
    use winreg::enums::*;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let key = hklm.open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion");

    let mut product_name = "Windows".to_string();
    let mut display_version = String::new();
    let mut build = String::new();
    let mut ubr = String::new();

    if let Ok(k) = key {
        product_name = k.get_value("ProductName").unwrap_or(product_name);
        display_version = k
            .get_value::<String, _>("DisplayVersion")
            .or_else(|_| k.get_value::<String, _>("ReleaseId"))
            .unwrap_or_default();
        build = k.get_value("CurrentBuildNumber").unwrap_or_default();
        let ubr_val: u32 = k.get_value("UBR").unwrap_or(0);
        ubr = ubr_val.to_string();
    }

    if build.is_empty() {
        product_name
    } else {
        format!("{product_name} {display_version} (Build {build}.{ubr})")
    }
}

#[cfg(not(windows))]
fn get_windows_version() -> String {
    "非 Windows 系統".to_string()
}

// ---------------------------------------------------------------------
// sys.session 產生
// ---------------------------------------------------------------------

fn write_session_file(dest: &Path, drivers: &[DriverEntry]) -> std::io::Result<()> {
    let win_ver = get_windows_version();
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    let mut content = String::new();
    content.push_str("===== 驅動程式匯出紀錄 (sys.session) =====\r\n");
    content.push_str(&format!("Windows 版本：{win_ver}\r\n"));
    content.push_str(&format!("匯出時間：{now}\r\n"));
    content.push_str(&format!("總共驅動：{}\r\n", drivers.len()));
    content.push_str("驅動列表：\r\n");
    for (i, d) in drivers.iter().enumerate() {
        content.push_str(&format!("  {:>3}. {}\r\n", i + 1, d.inf_name));
    }

    std::fs::write(dest.join("sys.session"), content)
}

// ---------------------------------------------------------------------
// GUI
// ---------------------------------------------------------------------

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([860.0, 620.0])
            .with_min_inner_size([700.0, 480.0]),
        ..Default::default()
    };

    eframe::run_native(
        "驅動程式備份工具",
        options,
        Box::new(|cc| {
            setup_fonts(&cc.egui_ctx);
            setup_style(&cc.egui_ctx);
            Box::new(DriverApp::default())
        }),
    )
}

/// 內嵌微軟正黑體 (msjh.ttf)，解決中文字型遺失顯示成空白方框(tofu)的問題。
/// 字體直接編譯進執行檔，使用者電腦不需另外安裝字型。
fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    fonts.font_data.insert(
        "msjh".to_owned(),
        egui::FontData::from_static(include_bytes!("../assets/msjh.ttf")),
    );

    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "msjh".to_owned());

    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .push("msjh".to_owned());

    ctx.set_fonts(fonts);
}

fn setup_style(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::dark();

    let accent = egui::Color32::from_rgb(88, 166, 255);
    style.visuals.selection.bg_fill = accent;
    style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(45, 45, 55);
    style.visuals.widgets.active.bg_fill = accent;
    style.visuals.window_fill = egui::Color32::from_rgb(24, 24, 30);
    style.visuals.panel_fill = egui::Color32::from_rgb(24, 24, 30);
    style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(38, 38, 46);
    style.visuals.widgets.inactive.rounding = egui::Rounding::same(8.0);
    style.visuals.widgets.hovered.rounding = egui::Rounding::same(8.0);
    style.visuals.widgets.active.rounding = egui::Rounding::same(8.0);
    style.visuals.window_rounding = egui::Rounding::same(12.0);
    style.visuals.window_shadow = egui::epaint::Shadow {
        offset: egui::vec2(0.0, 6.0),
        blur: 24.0,
        spread: 0.0,
        color: egui::Color32::from_black_alpha(120),
    };
    style.visuals.window_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(55, 55, 66));
    style.visuals.widgets.noninteractive.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(45, 45, 54));
    style.visuals.extreme_bg_color = egui::Color32::from_rgb(18, 18, 23);

    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(14.0, 8.0);
    style.spacing.window_margin = egui::Margin::same(16.0);

    use egui::{FontFamily, FontId, TextStyle};
    style.text_styles = [
        (TextStyle::Heading, FontId::new(22.0, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(15.0, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(15.0, FontFamily::Proportional)),
        (TextStyle::Small, FontId::new(12.0, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(13.5, FontFamily::Monospace)),
    ]
    .into();

    ctx.set_style(style);
}

/// 統一的卡片外框：深色底、圓角、細邊框，讓各區塊看起來像獨立面板
fn card<R>(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::none()
        .fill(egui::Color32::from_rgb(30, 30, 37))
        .rounding(egui::Rounding::same(10.0))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(50, 50, 60)))
        .inner_margin(egui::Margin::same(16.0))
        .show(ui, add_contents)
        .inner
}

impl eframe::App for DriverApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_worker();
        if self.rx.is_some() {
            ctx.request_repaint();
        }

        // 頂部分頁列
        egui::TopBottomPanel::top("top_bar")
            .frame(egui::Frame::none()
                .fill(egui::Color32::from_rgb(20, 20, 26))
                .inner_margin(egui::Margin::symmetric(14.0, 10.0)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("🛠").size(24.0));
                    ui.label(
                        egui::RichText::new("驅動程式備份工具")
                            .size(20.0)
                            .strong()
                            .color(egui::Color32::from_rgb(230, 230, 235)),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let (dot, text) = if self.export_running || matches!(self.import_stage, ImportStage::Running) {
                            (egui::Color32::from_rgb(255, 190, 90), "處理中")
                        } else {
                            (egui::Color32::from_rgb(120, 220, 140), "就緒")
                        };
                        ui.label(egui::RichText::new(text).weak());
                        ui.label(egui::RichText::new("●").color(dot));
                    });
                });
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    let accent = egui::Color32::from_rgb(88, 166, 255);
                    let make_tab = |ui: &mut egui::Ui, selected: bool, label: &str| -> egui::Response {
                        let mut btn = egui::Button::new(
                            egui::RichText::new(label).size(15.0).color(if selected {
                                egui::Color32::WHITE
                            } else {
                                egui::Color32::from_rgb(180, 180, 190)
                            }),
                        )
                        .rounding(egui::Rounding::same(8.0))
                        .min_size(egui::vec2(140.0, 34.0));
                        btn = if selected {
                            btn.fill(accent)
                        } else {
                            btn.fill(egui::Color32::from_rgb(32, 32, 40))
                        };
                        ui.add(btn)
                    };
                    if make_tab(ui, self.tab == Tab::Export, "📤  匯出驅動").clicked() {
                        self.tab = Tab::Export;
                    }
                    if make_tab(ui, self.tab == Tab::Import, "📥  匯入驅動").clicked() {
                        self.tab = Tab::Import;
                    }
                });
            });

        // 底部 Log 面板
        egui::TopBottomPanel::bottom("log_panel")
            .frame(egui::Frame::none()
                .fill(egui::Color32::from_rgb(16, 16, 21))
                .inner_margin(egui::Margin::symmetric(14.0, 10.0))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(40, 40, 48))))
            .resizable(true)
            .default_height(210.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("📋 執行紀錄").strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("清除").clicked() {
                            self.log_lines.clear();
                        }
                    });
                });
                ui.add_space(4.0);
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .max_height(160.0)
                    .show(ui, |ui| {
                        for line in &self.log_lines {
                            let color = if line.contains('❌') {
                                egui::Color32::from_rgb(255, 110, 110)
                            } else if line.contains('⚠') {
                                egui::Color32::from_rgb(255, 190, 90)
                            } else if line.contains('✅') {
                                egui::Color32::from_rgb(120, 220, 140)
                            } else {
                                egui::Color32::from_rgb(200, 200, 208)
                            };
                            ui.label(
                                egui::RichText::new(line)
                                    .color(color)
                                    .font(egui::FontId::new(13.0, egui::FontFamily::Monospace)),
                            );
                        }
                    });
            });

        // 主內容
        egui::CentralPanel::default().show(ctx, |ui| match self.tab {
            Tab::Export => self.ui_export(ui),
            Tab::Import => self.ui_import(ui, ctx),
        });
    }
}

impl DriverApp {
    fn ui_export(&mut self, ui: &mut egui::Ui) {
        ui.add_space(10.0);
        card(ui, |ui| {
            ui.label(
                egui::RichText::new("將目前電腦上所有第三方驅動程式匯出成備份")
                    .size(16.0)
                    .strong(),
            );
            ui.add_space(14.0);

            ui.horizontal(|ui| {
                if ui.add(egui::Button::new("📁  選擇匯出資料夾").min_size(egui::vec2(160.0, 32.0))).clicked() {
                    if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                        self.export_path = Some(folder);
                    }
                }
                let text = match &self.export_path {
                    Some(p) => p.display().to_string(),
                    None => "尚未選擇".to_string(),
                };
                ui.label(egui::RichText::new(text).monospace().weak());
            });

            ui.add_space(20.0);

            ui.add_enabled_ui(!self.export_running && self.export_path.is_some(), |ui| {
                let btn = egui::Button::new(
                    egui::RichText::new("開始匯出").size(16.0).color(egui::Color32::WHITE),
                )
                .fill(egui::Color32::from_rgb(88, 166, 255))
                .rounding(egui::Rounding::same(8.0));
                if ui.add_sized([180.0, 42.0], btn).clicked() {
                    self.start_export();
                }
            });

            if self.export_running {
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("匯出中，請稍候…（依驅動數量可能需要數分鐘）");
                });
            }
        });

        ui.add_space(16.0);
        card(ui, |ui| {
            ui.label(
                egui::RichText::new("📄 完成後會自動產生 sys.session")
                    .strong(),
            );
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("內含 Windows 版本、匯出時間、總驅動數與完整驅動清單。")
                    .weak(),
            );
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("⚠ 請確認本程式以「系統管理員」身分執行，否則匯出會直接失敗。")
                    .color(egui::Color32::from_rgb(255, 190, 90)),
            );
        });
    }

    fn ui_import(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.add_space(10.0);
        card(ui, |ui| {
            ui.label(
                egui::RichText::new("匯入單一驅動 (.inf) 或整個資料夾內的所有驅動")
                    .size(16.0)
                    .strong(),
            );
            ui.add_space(14.0);

            ui.horizontal(|ui| {
                if ui.add(egui::Button::new("📄  選擇單一驅動檔 (.inf)").min_size(egui::vec2(180.0, 32.0))).clicked() {
                    if let Some(f) = rfd::FileDialog::new().add_filter("驅動檔", &["inf"]).pick_file() {
                        self.import_target = Some(f);
                        self.import_is_folder = false;
                    }
                }
                if ui.add(egui::Button::new("📁  選擇整個驅動資料夾").min_size(egui::vec2(180.0, 32.0))).clicked() {
                    if let Some(f) = rfd::FileDialog::new().pick_folder() {
                        self.import_target = Some(f);
                        self.import_is_folder = true;
                    }
                }
            });

            let text = match &self.import_target {
                Some(p) => format!("已選擇：{}", p.display()),
                None => "尚未選擇".to_string(),
            };
            ui.label(egui::RichText::new(text).monospace().weak());

            ui.add_space(20.0);

            let running = matches!(self.import_stage, ImportStage::Running);
            ui.add_enabled_ui(!running && self.import_target.is_some(), |ui| {
                let btn = egui::Button::new(
                    egui::RichText::new("開始匯入").size(16.0).color(egui::Color32::WHITE),
                )
                .fill(egui::Color32::from_rgb(88, 166, 255))
                .rounding(egui::Rounding::same(8.0));
                if ui.add_sized([180.0, 42.0], btn).clicked() {
                    self.start_import_scan();
                }
            });

            if running {
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("處理中，請稍候…");
                });
            }
        });

        ui.add_space(16.0);
        card(ui, |ui| {
            ui.label(egui::RichText::new("⚠ 請確認本程式以「系統管理員」身分執行。").color(egui::Color32::from_rgb(255, 190, 90)));
        });

        // 覆蓋確認彈窗
        if let ImportStage::Confirming(dups) = &self.import_stage {
            let dups_clone = dups.clone();
            let all_infs = collect_inf_files(self.import_target.as_ref().unwrap());
            let mut choice: Option<bool> = None;

            egui::Window::new("偵測到重複的驅動程式")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(ctx, |ui| {
                    ui.label(format!("有 {} 個驅動已存在於系統中：", dups_clone.len()));
                    egui::ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
                        for (path, old) in &dups_clone {
                            ui.label(format!("• {}（現有：{}）", path.display(), old.published_name));
                        }
                    });
                    ui.add_space(10.0);
                    ui.label("是否要覆蓋目前已安裝的驅動版本？");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("✅ 覆蓋安裝").clicked() {
                            choice = Some(true);
                        }
                        if ui.button("⏭ 略過重複，僅安裝其餘").clicked() {
                            choice = Some(false);
                        }
                        if ui.button("✖ 取消匯入").clicked() {
                            choice = Some(false);
                            self.import_stage = ImportStage::Idle;
                        }
                    });
                });

            if let Some(c) = choice {
                if !matches!(self.import_stage, ImportStage::Idle) {
                    self.confirm_import_choice(c, dups_clone, all_infs);
                }
            }
        }
    }
}
