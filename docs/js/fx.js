
(()=>{
const $=(s,r=document)=>r.querySelector(s),$$=(s,r=document)=>[...r.querySelectorAll(s)];
// reveal on scroll
const io=new IntersectionObserver(es=>es.forEach(e=>{if(e.isIntersecting){e.target.classList.add('in');io.unobserve(e.target)}}),{threshold:.12});
$$('.reveal').forEach((el,i)=>{el.style.transitionDelay=(i%4)*70+'ms';io.observe(el)});
// cursor glow + card spotlight + tilt
const glow=$('.glow');
addEventListener('pointermove',e=>{if(glow){glow.style.left=e.clientX+'px';glow.style.top=e.clientY+'px'}});
$$('.card').forEach(c=>{
 c.addEventListener('pointermove',e=>{const r=c.getBoundingClientRect(),x=e.clientX-r.left,y=e.clientY-r.top;
  c.style.setProperty('--mx',x+'px');c.style.setProperty('--my',y+'px');
  c.style.transform=`perspective(800px) rotateX(${((y/r.height)-.5)*-8}deg) rotateY(${((x/r.width)-.5)*8}deg) translateY(-4px)`});
 c.addEventListener('pointerleave',()=>c.style.transform='');
});
// counters
$$('[data-count]').forEach(el=>{const t=+el.dataset.count;let n=0;const go=()=>{n+=Math.max(1,t/40);if(n>=t){el.textContent=t+(el.dataset.suf||'');return}el.textContent=Math.floor(n)+(el.dataset.suf||'');requestAnimationFrame(go)};
 new IntersectionObserver((es,o)=>{if(es[0].isIntersecting){go();o.disconnect()}}).observe(el)});
// toast helper
window.toast=m=>{let t=$('.toast');if(!t){t=document.createElement('div');t.className='toast';document.body.append(t)}t.textContent=m;t.classList.add('show');clearTimeout(t._h);t._h=setTimeout(()=>t.classList.remove('show'),2200)};
// background canvas
const cv=$('#bg');if(!cv)return;const ctx=cv.getContext('2d');let W,H,mode=cv.dataset.mode,col=getComputedStyle(document.documentElement).getPropertyValue('--a').trim(),col2=getComputedStyle(document.documentElement).getPropertyValue('--a2').trim(),col3=getComputedStyle(document.documentElement).getPropertyValue('--a3').trim();
const rs=()=>{W=cv.width=innerWidth;H=cv.height=innerHeight};rs();addEventListener('resize',rs);
let mx=.5,my=.5;addEventListener('pointermove',e=>{mx=e.clientX/W;my=e.clientY/H});
const P=[];
if(mode==='aurora'||mode==='nodes'){for(let i=0;i<70;i++)P.push({x:Math.random()*W,y:Math.random()*H,vx:(Math.random()-.5)*.5,vy:(Math.random()-.5)*.5,r:Math.random()*2+1})}
const drops=[];const cw=18;
const chars='01abcdef0123456789SHA512';
function frame(t){
 ctx.clearRect(0,0,W,H);
 if(mode==='aurora'){
  for(let k=0;k<3;k++){const g=ctx.createRadialGradient(W*(.2+.3*k+.1*Math.sin(t/3000+k)+(mx-.5)*.1),H*(.3+.2*Math.cos(t/2500+k)),0,W*(.2+.3*k),H*.4,W*.45);
   g.addColorStop(0,[col,col2,col3][k]+'40');g.addColorStop(1,'transparent');ctx.fillStyle=g;ctx.fillRect(0,0,W,H)}
  ctx.fillStyle=col;P.forEach(p=>{p.x+=p.vx;p.y+=p.vy;if(p.x<0||p.x>W)p.vx*=-1;if(p.y<0||p.y>H)p.vy*=-1;ctx.globalAlpha=.5;ctx.beginPath();ctx.arc(p.x,p.y,p.r,0,7);ctx.fill()});
  ctx.globalAlpha=.12;ctx.strokeStyle=col;for(let i=0;i<P.length;i++)for(let j=i+1;j<P.length;j++){const dx=P[i].x-P[j].x,dy=P[i].y-P[j].y;if(dx*dx+dy*dy<14000){ctx.beginPath();ctx.moveTo(P[i].x,P[i].y);ctx.lineTo(P[j].x,P[j].y);ctx.stroke()}}ctx.globalAlpha=1;
 }else if(mode==='matrix'){
  const cols=Math.ceil(W/cw);while(drops.length<cols)drops.push(Math.random()*-50);
  ctx.fillStyle='rgba(0,0,0,.0)';ctx.font='15px monospace';
  drops.forEach((d,i)=>{const y=d*cw;ctx.fillStyle=col;ctx.globalAlpha=.55;ctx.fillText(chars[(Math.random()*chars.length)|0],i*cw,y);
   ctx.fillStyle=col2;ctx.globalAlpha=.18;for(let k=1;k<12;k++)ctx.fillText(chars[(i*7+k+(t/200|0))%chars.length],i*cw,y-k*cw);
   if(t%2<1)drops[i]+=.12+((i*13)%7)/60;if(y>H+200&&Math.random()>.97)drops[i]=Math.random()*-20});ctx.globalAlpha=1;
 }else if(mode==='grid'){
  const hz=H*.55;ctx.strokeStyle=col;ctx.globalAlpha=.25;ctx.lineWidth=1;
  for(let i=-30;i<=30;i++){ctx.beginPath();ctx.moveTo(W/2+i*20+(mx-.5)*200,hz);ctx.lineTo(W/2+i*160,H);ctx.stroke()}
  const off=(t/40)%1;for(let j=0;j<18;j++){const k=(j+off)/18,y=hz+(H-hz)*k*k;ctx.globalAlpha=.1+.3*k;ctx.beginPath();ctx.moveTo(0,y);ctx.lineTo(W,y);ctx.stroke()}
  const g=ctx.createRadialGradient(W/2,hz,0,W/2,hz,W*.5);g.addColorStop(0,col2+'55');g.addColorStop(1,'transparent');ctx.globalAlpha=1;ctx.fillStyle=g;ctx.fillRect(0,0,W,H);
 }else if(mode==='voxel'){
  if(!P.length){for(let i=0;i<46;i++)P.push({x:Math.random()*W,y:Math.random()*H,s:8+Math.random()*26,v:.1+Math.random()*.4,r:Math.random()*6,c:Math.random()})}
  P.forEach(p=>{p.y-=p.v;p.r+=.004;if(p.y<-60){p.y=H+60;p.x=Math.random()*W}
   ctx.save();ctx.translate(p.x,p.y);ctx.rotate(p.r);ctx.globalAlpha=.16;ctx.fillStyle=p.c>.5?col:col2;ctx.fillRect(-p.s/2,-p.s/2,p.s,p.s);ctx.strokeStyle=col;ctx.globalAlpha=.4;ctx.strokeRect(-p.s/2,-p.s/2,p.s,p.s);ctx.restore()});
 }else if(mode==='sparks'){
  if(!P.length){for(let i=0;i<90;i++)P.push({x:Math.random()*W,y:Math.random()*H,v:.3+Math.random()*1.2,r:Math.random()*2.2+.6,w:Math.random()*6})}
  P.forEach(p=>{p.y-=p.v;p.x+=Math.sin(t/700+p.w)*.5;if(p.y<-10){p.y=H+10;p.x=Math.random()*W}ctx.globalAlpha=.25+.5*Math.abs(Math.sin(t/500+p.w));ctx.fillStyle=p.w>3?col:col2;ctx.shadowColor=col;ctx.shadowBlur=12;ctx.beginPath();ctx.arc(p.x,p.y,p.r,0,7);ctx.fill()});ctx.shadowBlur=0;ctx.globalAlpha=1;
 }
 requestAnimationFrame(frame)}
requestAnimationFrame(frame);
})();


// ===== 最新 Release:抓 GitHub API,填入版本、日期、檔案清單,失敗則維持連結 =====
(()=>{
const $=(s,r=document)=>r.querySelector(s),$$=(s,r=document)=>[...r.querySelectorAll(s)];
// 搜尋過濾(功能頁)
const f=$('#filter');
if(f)f.addEventListener('input',()=>{const q=f.value.trim().toLowerCase();$$('#items .card,#items tr.f').forEach(c=>c.hidden=!!q&&!c.textContent.toLowerCase().includes(q))});
const repo=document.body.dataset.repo;if(!repo)return;
const fmt=n=>n>1048576?(n/1048576).toFixed(1)+' MB':Math.max(1,Math.round(n/1024))+' KB';
const apply=(j,pre)=>{
 $$('[data-tag]').forEach(e=>e.textContent=j.tag_name||'');
 $$('[data-date]').forEach(e=>e.textContent=(pre?'(Pre-release) ':'')+(j.published_at?new Date(j.published_at).toLocaleDateString('zh-TW'):''));
 if(pre&&j.html_url)$$('[data-dl]').forEach(a=>a.href=j.html_url);
 $$('[data-assets]').forEach(box=>{
  const as=(j.assets||[]);
  box.innerHTML=as.length?as.map(a=>'<a class="btn s" href="'+a.browser_download_url+'" rel="noopener">⬇ '+a.name.replace(/</g,'&lt;')+' <small>'+fmt(a.size)+'</small></a>').join(''):'<span style="color:var(--mut)">此版本沒有附加檔案,請從 Release 頁面查看原始碼壓縮檔。</span>';
 });
};
const key='rel:'+repo;let c=null;try{c=JSON.parse(sessionStorage.getItem(key))}catch(e){}
if(c){apply(c.j,c.pre);return}
const api='https://api.github.com/repos/'+repo;
fetch(api+'/releases/latest').then(r=>r.ok?r.json():Promise.reject()).then(j=>({j,pre:false}))
 // 沒有正式版時(例如只發佈 Pre-release),改抓最新一筆
 .catch(()=>fetch(api+'/releases?per_page=1').then(r=>r.ok?r.json():Promise.reject()).then(a=>a[0]?{j:a[0],pre:true}:Promise.reject()))
 .then(o=>{try{sessionStorage.setItem(key,JSON.stringify(o))}catch(e){}apply(o.j,o.pre)})
 .catch(()=>{$$('[data-assets]').forEach(b=>b.innerHTML='<span style="color:var(--mut)">暫時無法取得版本資訊,請直接前往 Release 頁面。</span>')});
})();
