// Redact GUI v0.4 — vanilla JS, gọi Rust commands qua Tauri invoke.
// Gate khai loại (00) + auto-scan dấu hiệu mật + 3 tab.

const { invoke } = window.__TAURI__.core;
const { open } = window.__TAURI__.dialog;
const { revealItemInDir } = window.__TAURI__.opener;

// ─── state ───
const state = {
  redactPath: null,
  unredactPath: null,
  dictPath: null,
  docClassId: null,      // khai loại tài liệu
  classAction: null,     // Block | RequireApproval | AllowedAfterRedact
  scanned: false,        // đã auto-scan file đang chọn
  scanClean: false,      // scan xong không có dấu hiệu
};

// ─── helpers ───
const $ = (id) => document.getElementById(id);

function showError(msg) {
  const box = $('error-box');
  box.textContent = msg;
  box.hidden = false;
}
function clearError() { $('error-box').hidden = true; }

async function pickFile(filters) {
  const picked = await open({ multiple: false, directory: false, filters });
  return typeof picked === 'string' ? picked : null;
}

function markPicked(spanId, path, placeholder) {
  const el = $(spanId);
  el.textContent = path || placeholder;
  el.classList.toggle('picked', !!path);
}

// ─── tabs + slider ───
const TABS = ['redact', 'unredact', 'law'];
function moveSlider(tabEl) {
  const s = $('tab-slider');
  s.style.left = tabEl.offsetLeft + 'px';
  s.style.width = tabEl.offsetWidth + 'px';
}
for (const name of TABS) {
  $(`tab-${name}`).addEventListener('click', () => {
    for (const n of TABS) {
      $(`tab-${n}`).classList.toggle('active', n === name);
      $(`panel-${n}`).classList.toggle('active', n === name);
      $(`tab-${n}`).setAttribute('aria-selected', n === name);
    }
    moveSlider($(`tab-${name}`));
  });
}
window.addEventListener('resize', () => moveSlider(document.querySelector('.tab.active')));

// ─── gate khai loại (00) ───
let CLASSES = [];
async function initDocClass() {
  CLASSES = await invoke('list_classes');
  const row = $('doc-class-row');
  row.innerHTML = '';
  for (const c of CLASSES) {
    const label = document.createElement('label');
    label.className = 'class-card';
    label.dataset.id = c.id;
    const hint =
      c.action === 'Block' ? 'DỪNG tuyệt đối' :
      c.action === 'RequireApproval' ? 'Cần phê duyệt' : 'Redact rồi dùng';
    label.innerHTML = `<input type="radio" name="docclass" value="${c.id}"/>
      <div class="cc-name">${c.label}</div><div class="cc-hint">${hint}</div>`;
    label.querySelector('input').addEventListener('change', () => selectClass(c));
    row.appendChild(label);
  }
}
function selectClass(c) {
  state.docClassId = c.id;
  document.getElementById('gate-scene').hidden = true; // đã chọn → thu animation
  state.classAction = c.action;
  document.querySelectorAll('.class-card').forEach((el) => {
    el.classList.toggle('selected', el.dataset.id === c.id);
    el.classList.toggle('blocked', c.action === 'Block' && el.dataset.id === c.id);
  });
  const msg = $('gate-msg');
  const consent = $('consent-approval');
  $('chk-approval').checked = false;
  if (c.action === 'Block') {
    msg.hidden = false;
    msg.className = 'gate-msg block';
    msg.textContent = `${c.action_label} — ${c.rule}`;
    consent.hidden = true;
  } else if (c.action === 'RequireApproval') {
    msg.hidden = false;
    msg.className = 'gate-msg';
    msg.textContent = `${c.action_label}. ${c.rule}`;
    consent.hidden = false;
  } else {
    msg.hidden = false;
    msg.className = 'gate-msg';
    msg.textContent = `${c.action_label}. ${c.rule}`;
    consent.hidden = true;
  }
  refreshRedactBtn();
}

// ─── nút Redact enabled khi: class hợp lệ + approval tick (nếu cần) + file + scan consent (nếu có dấu hiệu) ───
function refreshRedactBtn() {
  let ok = !!state.redactPath && !!state.docClassId;
  if (ok && state.classAction === 'Block') ok = false;
  if (ok && state.classAction === 'RequireApproval' && !$('chk-approval').checked) ok = false;
  if (ok && state.scanned && !state.scanClean && !$('chk-scan').checked) ok = false;
  $('btn-redact').disabled = !ok;
}

// ─── chọn file / thư mục + trigger auto-scan ───
async function setRedactPath(path, label) {
  state.redactPath = path;
  markPicked('redact-file-name', label || path, 'Chưa chọn file');
  await runScan(path);
  refreshRedactBtn();
}

$('btn-pick-redact').addEventListener('click', async () => {
  clearError();
  const path = await pickFile([
    { name: 'Tài liệu', extensions: ['txt', 'csv', 'md', 'log', 'json', 'xml', 'pdf', 'docx', 'xlsx', 'pptx'] },
  ]);
  if (path) await setRedactPath(path);
});

$('btn-pick-folder').addEventListener('click', async () => {
  clearError();
  const picked = await window.__TAURI__.dialog.open({ multiple: false, directory: true });
  const path = typeof picked === 'string' ? picked : null;
  if (path) await setRedactPath(path, path + ' (cả thư mục con)');
});

// ─── F3: auto-scan dấu hiệu mật ───
async function runScan(path) {
  state.scanned = false;
  state.scanClean = false;
  $('chk-scan').checked = false;
  const box = $('scan-box');
  box.hidden = true;
  if (!path) return;
  try {
    const r = await invoke('scan_secret', { path });
    state.scanned = true;
    if (r.markers.length > 0) {
      $('scan-title').textContent =
        `Phát hiện ${r.markers.length} dấu hiệu tài liệu mật (quét ${r.files_scanned} file) — xác nhận trước khi tiếp tục:`;
      const ul = $('scan-markers');
      ul.innerHTML = '';
      for (const m of r.markers) {
        const li = document.createElement('li');
        li.innerHTML = `<strong>${m.phrase}</strong> — ${m.note}`;
        ul.appendChild(li);
      }
      box.hidden = false;
    } else {
      state.scanClean = true;
    }
  } catch (e) {
    // scan là lớp phụ — lỗi scan không chặn redact
    state.scanClean = true;
  }
}
$('chk-scan').addEventListener('change', refreshRedactBtn);
$('chk-approval').addEventListener('change', refreshRedactBtn);

// ─── examples ───
async function initExamples() {
  try {
    const names = await invoke('list_examples');
    const row = $('examples-row');
    row.querySelectorAll('.chip').forEach((c) => c.remove());
    for (const name of names) {
      const chip = document.createElement('button');
      chip.className = 'chip';
      chip.textContent = name.replace(/^vi_du_/, '').replace(/\.[a-z]+$/, '');
      chip.title = name;
      chip.addEventListener('click', async () => {
        clearError();
        try {
          const path = await invoke('load_example', { name });
          await setRedactPath(path);
        } catch (e) {
          showError(String(e));
        }
      });
      row.appendChild(chip);
    }
  } catch (e) {
    $('examples-row').hidden = true;
  }
}

// ─── redact ───
$('btn-redact').addEventListener('click', async () => {
  if (!state.redactPath) return;
  clearError();
  $('btn-redact').disabled = true;
  $('btn-redact').textContent = 'Đang xử lý…';
  try {
    const level = Number(document.querySelector('input[name="level"]:checked').value);
    const r = await invoke('redact_file', { path: state.redactPath, level });
    $('redact-stats').innerHTML =
      `<strong>${r.total}</strong> thông tin đã được thay bằng placeholder` +
      (r.per_pattern.length ? ' — ' + r.per_pattern.map(([k, v]) => `${k} ${v}`).join(', ') : '');
    $('redact-out-path').textContent = r.redacted_path;
    $('redact-dict-path').textContent = r.dict_path;
    if (r.is_folder) {
      $('folder-summary').textContent =
        `${r.files_ok} file đã redact` + (r.files_skipped ? ` · ${r.files_skipped} file bỏ qua` : '');
      $('folder-summary-row').hidden = false;
    } else {
      $('folder-summary-row').hidden = true;
    }
    $('redact-result').hidden = false;
  } catch (e) {
    showError(String(e));
  } finally {
    refreshRedactBtn();
    $('btn-redact').textContent = 'Redact file';
  }
});

// ─── unredact ───
function refreshUnredactBtn() {
  $('btn-unredact').disabled = !(state.unredactPath && state.dictPath);
}

$('btn-pick-unredact').addEventListener('click', async () => {
  clearError();
  const path = await pickFile([{ name: 'Tất cả', extensions: ['*'] }]);
  if (path) {
    state.unredactPath = path;
    markPicked('unredact-file-name', path, 'Chưa chọn file');
    refreshUnredactBtn();
  }
});

$('btn-pick-dict').addEventListener('click', async () => {
  clearError();
  const path = await pickFile([{ name: 'File dict', extensions: ['json'] }]);
  if (path) {
    state.dictPath = path;
    markPicked('dict-file-name', path, 'Chưa chọn dict');
    refreshUnredactBtn();
  }
});

$('btn-unredact').addEventListener('click', async () => {
  if (!state.unredactPath || !state.dictPath) return;
  clearError();
  $('btn-unredact').disabled = true;
  $('btn-unredact').textContent = 'Đang khôi phục…';
  try {
    const r = await invoke('unredact_file', {
      path: state.unredactPath,
      dictPath: state.dictPath,
    });
    $('unredact-stats').innerHTML =
      `Đã khôi phục <strong>${r.restored_count}</strong> placeholder từ dict ${r.dict_size} entry`;
    $('unredact-out-path').textContent = r.restored_path;
    $('unredact-result').hidden = false;
  } catch (e) {
    showError(String(e));
  } finally {
    refreshUnredactBtn();
    $('btn-unredact').textContent = 'Khôi phục dữ liệu';
  }
});

// ─── tab Quy định ───
async function initLaw() {
  const classes = await invoke('list_classes');
  const wrap = $('law-classes');
  wrap.innerHTML = '';
  for (const c of classes) {
    const card = document.createElement('div');
    card.className = 'law-card';
    const cls = c.action === 'Block' ? 'block' : c.action === 'RequireApproval' ? 'approval' : 'ok';
    card.innerHTML = `
      <div class="lc-name">${c.label}</div>
      <div class="lc-action ${cls}">${c.action_label}</div>
      <div class="lc-rule">${c.rule}</div>
      <div class="lc-basis">${c.legal_basis} · <a href="${c.source}" target="_blank">xem văn bản gốc ↗</a></div>`;
    wrap.appendChild(card);
  }
}

// ─── reveal ───
for (const btn of document.querySelectorAll('.btn-reveal')) {
  btn.addEventListener('click', () => {
    const path = $(btn.dataset.reveal).textContent;
    if (path) revealItemInDir(path);
  });
}

// ─── init ───
initDocClass();
initExamples();
initLaw();
$('law-source-link').addEventListener('click', (e) => {
  e.preventDefault();
  window.__TAURI__.opener.openUrl(
    'https://locdang.com/che-du-lieu-truoc-khi-ua-len-ai-thi-luat-cho-phep-en-au/');
});
moveSlider($('tab-redact'));
