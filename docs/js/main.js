(function(){
  var page=location.pathname.split('/').pop()||'index.html';
  document.querySelectorAll('nav a[href]').forEach(function(a){if(a.getAttribute('href')===page)a.classList.add('on')});
  var f=document.getElementById('filter');
  if(f){f.addEventListener('input',function(){
    var q=f.value.trim().toLowerCase();
    document.querySelectorAll('#items .card').forEach(function(c){c.hidden=q&&c.textContent.toLowerCase().indexOf(q)<0});
  })}
})();
