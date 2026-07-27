fn main() {
    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        // 內嵌 manifest，要求以系統管理員身分執行（匯出/匯入驅動需要 Admin 權限）
        res.set_manifest(
            r#"
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="requireAdministrator" uiAccess="false" />
      </requestedPrivileges>
    </security>
  </trustInfo>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true</dpiAware>
    </windowsSettings>
  </application>
</assembly>
"#,
        );
        res.compile().expect("無法編譯 Windows 資源(manifest)");
    }
}
