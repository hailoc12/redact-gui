// Redact GUI — vanilla JS, gọi Rust commands qua Tauri invoke.
// Không framework, không network — mọi xử lý nằm ở Rust side (100% local).

const { invoke } = window.__TAURI__.core;
const { open } = window.__TAURI__.dialog;
const { revealItemInDir } = window.__TAURI__.opener;

// ─── state ───
const state = {
  redactPath: null,
  unredactPath: null,
  dictPath: null,
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

// ─── tabs ───
for (const name of ['redact', 'unredact']) {
  $(`tab-${name}`).addEventListener('click', () => {
    for (const n of ['redact', 'unredact']) {
      $(`tab-${n}`).classList.toggle('active', n === name);
      $(`panel-${n}`).classList.toggle('active', n === name);
      $(`tab-${n}`).setAttribute('aria-selected', n === name);
    }
  });
}

// ─── redact flow ───
$('btn-pick-redact').addEventListener('click', async () => {
  clearError();
  const path = await pickFile([
    { name: 'Tài liệu', extensions: ['txt', 'csv', 'md', 'log', 'json', 'pdf'] },
  ]);
  if (path) {
    state.redactPath = path;
    markPicked('redact-file-name', path, 'Chưa chọn file');
    $('btn-redact').disabled = false;
  }
});

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
      (r.per_pattern.length
        ? ' — ' + r.per_pattern.map(([k, v]) => `${k} ${v}`).join(', ')
        : '');
    $('redact-out-path').textContent = r.redacted_path;
    $('redact-dict-path').textContent = r.dict_path;
    $('redact-result').hidden = false;
  } catch (e) {
    showError(String(e));
  } finally {
    $('btn-redact').disabled = false;
    $('btn-redact').textContent = 'Redact file';
  }
});

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
          state.redactPath = path;
          markPicked('redact-file-name', path, 'Chưa chọn file');
          $('btn-redact').disabled = false;
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
initExamples();

// ─── unredact flow ───
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
  const path = await pickFile([
    { name: 'File dict', extensions: ['json'] },
  ]);
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

// ─── reveal in finder/explorer ───
for (const btn of document.querySelectorAll('.btn-reveal')) {
  btn.addEventListener('click', () => {
    const path = $(btn.dataset.reveal).textContent;
    if (path) revealItemInDir(path);
  });
}
