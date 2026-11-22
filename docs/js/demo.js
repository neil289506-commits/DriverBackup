
const L={e:['> 要求系統管理員權限… OK','pnputil /export-driver * D:\\Backup','已匯出 oem3.inf (Realtek Audio)','已匯出 oem7.inf (Intel Wi-Fi 6)','已匯出 oem12.inf (NVIDIA Display)','已匯出 oem15.inf (Intel Bluetooth)','寫入 sys.session:Windows 11 · 4 個驅動','✔ 匯出完成'],
 i:['> 掃描 D:\\Backup 內的 .inf…','pnputil /enum-drivers','偵測到重複:oem7.inf(依檔名比對)→ 詢問覆寫:是','pnputil /delete-driver oem7.inf /uninstall /force','pnputil /add-driver oem7.inf /install','pnputil /add-driver oem12.inf /install','✔ 匯入完成']};
let m='e',t=null;
const D={e:'選擇備份目的資料夾,按「Start Export」,資料夾內會有匯出的驅動與 sys.session。',i:'選擇單一 .inf 或含多個 .inf 的資料夾,按「Start Import」;若有同名驅動會詢問是否覆寫。'};
function sync(){dd.textContent=D[m];go.textContent=m==='e'?'Start Export':'Start Import';lg.textContent='';pb.style.width='0';st.textContent='';clearInterval(t);go.disabled=false}
document.querySelectorAll('.tab').forEach(b=>b.onclick=()=>{document.querySelectorAll('.tab').forEach(x=>x.classList.toggle('on',x===b));m=b.dataset.m;sync()});
go.onclick=()=>{sync();go.disabled=true;let i=0;const a=L[m];t=setInterval(()=>{lg.textContent+=a[i]+'\n';lg.scrollTop=1e5;i++;pb.style.width=(i/a.length*100)+'%';if(i>=a.length){clearInterval(t);go.disabled=false;st.textContent='完成'}},600)};
sync();
