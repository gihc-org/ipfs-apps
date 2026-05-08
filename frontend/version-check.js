(async () => {
  let initialCid = null;

  async function fetchCid() {
    try {
      const res = await fetch(location.pathname, { method: 'HEAD', cache: 'no-store' });
      return res.headers.get('X-Ipfs-Roots');
    } catch {
      return null;
    }
  }

  function showBanner() {
    if (document.getElementById('version-banner')) return;
    const banner = document.createElement('div');
    banner.id = 'version-banner';
    banner.style.cssText = [
      'position:fixed', 'bottom:0', 'left:0', 'right:0',
      'background:#2563eb', 'color:#fff',
      'padding:12px 20px',
      'display:flex', 'align-items:center', 'gap:16px',
      'z-index:9999', 'font-size:14px',
      'box-shadow:0 -2px 8px rgba(0,0,0,0.2)',
    ].join(';');
    banner.innerHTML =
      '<span style="flex:1">Ny version klar — genindlæs for at opdatere.</span>' +
      '<button onclick="location.reload()" style="background:#fff;color:#2563eb;border:none;padding:6px 16px;border-radius:4px;cursor:pointer;font-weight:600">Genindlæs</button>' +
      '<button onclick="this.parentElement.remove()" style="background:transparent;color:#fff;border:none;cursor:pointer;font-size:20px;line-height:1;padding:0 4px">×</button>';
    document.body.appendChild(banner);
  }

  initialCid = await fetchCid();

  setInterval(async () => {
    const cid = await fetchCid();
    if (cid && initialCid && cid !== initialCid) showBanner();
  }, 5 * 60 * 1000);
})();
