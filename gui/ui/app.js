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
const TABS = ['redact', 'unredact', 'log', 'law'];
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
  // mặc định: Tài liệu công khai
  const def = row.querySelector('input[value="public_document"]');
  if (def) { def.checked = true; def.dispatchEvent(new Event('change')); }
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
  const consentSpan = consent.querySelector('span');
  if (c.action === 'Block') {
    msg.hidden = false;
    msg.className = 'gate-msg block';
    msg.textContent = `${c.action_label} — ${c.rule}`;
    consent.hidden = true;
  } else if (c.action === 'RequireApproval') {
    msg.hidden = false;
    msg.className = 'gate-msg';
    msg.textContent = `${c.action_label}. ${c.rule}`;
    consentSpan.textContent = 'Tôi đã xin phê duyệt của cấp có thẩm quyền và cam kết tự chịu trách nhiệm khi xử lý tài liệu này.';
    consent.hidden = false;
  } else {
    msg.hidden = false;
    msg.className = 'gate-msg';
    msg.textContent = `${c.action_label}. ${c.rule}`;
    consentSpan.textContent = 'Tôi có quyền sử dụng tài liệu này (hoặc đã được phê duyệt) và cam kết tự chịu trách nhiệm khi che thông tin.';
    consent.hidden = false;
  }
  refreshRedactBtn();
}

// ─── nút Redact enabled khi: class hợp lệ + approval tick (nếu cần) + file + scan consent (nếu có dấu hiệu) ───
function refreshRedactBtn() {
  let ok = !!state.redactPath && !!state.docClassId;
  if (ok && state.classAction === 'Block') ok = false;
  if (ok && state.classAction !== 'Block' && !$('chk-approval').checked) ok = false;
  if (ok && state.scanned && !state.scanClean && !$('chk-scan').checked) ok = false;
  $('btn-redact').disabled = !ok;
  // F2: label đúng ngữ cảnh
  const isFolder = state.redactPath && String(state.redactPath).includes('(cả thư mục con)');
  if ($('btn-redact').textContent !== 'Đang xử lý…') {
    $('btn-redact').textContent = isFolder ? 'Redact cả thư mục' : 'Redact file';
  }
  // F1: hint nói rõ còn thiếu gì
  const hint = $('redact-hint');
  if (ok) {
    hint.hidden = true;
  } else {
    const missing = [];
    if (!state.redactPath) missing.push('chọn file/thư mục ở bước 01');
    if (!state.docClassId) missing.push('khai loại tài liệu ở bước 00');
    else if (state.classAction === 'Block') missing.push('nhóm này bị chặn — chọn nhóm khác');
    else if (!$('chk-approval').checked) missing.push('tick cam kết quyền sử dụng');
    if (state.scanned && !state.scanClean && !$('chk-scan').checked) missing.push('xác nhận cảnh báo mật ở bước 02');
    hint.textContent = 'Còn thiếu: ' + missing.join(' · ');
    hint.hidden = false;
  }
}

// ─── chọn file / thư mục + trigger auto-scan ───
async function setRedactPath(path, label) {
  if (state.redactPath !== path) {
    $('chk-approval').checked = false; // F3: consent gắn với TÀI LIỆU — đổi tài liệu phải cam kết lại
  }
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
  const statusEl = $('scan-status');
  if (!path) {
    statusEl.innerHTML = '<span class="scan-idle">Chọn tài liệu ở bước 01 — tool sẽ tự động rà quét dấu hiệu tài liệu mật.</span>';
    return;
  }
  statusEl.innerHTML = '<span class="scan-running">⟳ Đang rà quét nội dung…</span>';
  try {
    const r = await invoke('scan_secret', { path });
    state.scanned = true;
    if (r.markers.length > 0) {
      statusEl.innerHTML = `<span class="scan-hit">⚠ Phát hiện dấu hiệu tài liệu mật — cần xác nhận bên dưới.</span>`;
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
      statusEl.innerHTML = `<span class="scan-ok">✓ Không phát hiện dấu hiệu mật (đã rà quét ${r.files_scanned} file).</span>`;
    }
  } catch (e) {
    // scan là lớp phụ — lỗi scan không chặn redact
    state.scanClean = true;
    statusEl.innerHTML = '<span class="scan-idle">Không quét được nội dung (file nhị phân?) — tự kiểm tra thủ công.</span>';
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
    const r = await invoke('redact_file', { path: state.redactPath, level, docClass: state.docClassId || '-' });
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
  const ok = !!(state.unredactPath && state.dictPath);
  $('btn-unredact').disabled = !ok;
  const hint = $('unredact-hint');
  if (ok) { hint.hidden = true; return; }
  const missing = [];
  if (!state.unredactPath) missing.push('chọn file đã qua AI');
  if (!state.dictPath) missing.push('chọn file dict');
  hint.textContent = 'Còn thiếu: ' + missing.join(' · ');
  hint.hidden = false;
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


// ─── tab Log ───
async function refreshLog() {
  try {
    const info = await invoke('get_log_info');
    $('log-machine').textContent = info.machine;
    $('log-path').textContent = info.log_path;
    const chk = $('chk-org');
    chk.checked = info.org_feature && info.org_endpoint;
    $('org-status-text').textContent = info.org_feature
      ? (info.org_endpoint
          ? 'ĐANG BẬT — log gửi về API tổ chức (bản build org-mode)'
          : 'Build org-mode nhưng CHƯA cấu hình REDACT_ORG_LOG_URL — log chỉ lưu local')
      : 'TẮT — bản cá nhân: log chỉ lưu trên máy bạn, không gửi đi đâu';
    const wrap = $('log-entries');
    wrap.innerHTML = '';
    $('log-empty').hidden = info.entries.length > 0;
    for (const ev of info.entries.slice(0, 50)) {
      const row = document.createElement('div');
      row.className = 'log-row';
      row.innerHTML = `<span>${ev.timestamp.replace('T', ' ').replace('Z', '')}</span>
        <span>${ev.action}</span>
        <span class="log-file" title="${ev.file_name}">${ev.file_name}</span>
        <span>${ev.doc_class}</span>`;
      wrap.appendChild(row);
    }
  } catch (e) {
    console.error(e);
  }
}
$('tab-log').addEventListener('click', refreshLog);
$('btn-open-logfile').addEventListener('click', () => {
  const p = $('log-path').textContent;
  if (p && p !== '…') revealItemInDir(p);
});
$('btn-clear-log').addEventListener('click', async () => {
  await invoke('clear_log');
  refreshLog();
});

// ─── init ───
initDocClass();
initExamples();
initLaw();
$('law-source-link').addEventListener('click', (e) => {
  e.preventDefault();
  window.__TAURI__.opener.openUrl(
    'https://locdang.com/che-du-lieu-truoc-khi-ua-len-ai-thi-luat-cho-phep-en-au/');
});
requestAnimationFrame(() => moveSlider($('tab-redact')));
window.addEventListener('load', () => setTimeout(() => moveSlider(document.querySelector('.tab.active')), 400));
