'use strict';

// ---------------------------------------------------------------------------
// state
// ---------------------------------------------------------------------------

var state = {
  overview: null,
  config: null,
  tab: 'dashboard',
  logCursor: 0,
  logLines: [],
  logFollow: true,
  logLevel: 'ALL',
  logTimer: null,
  cabinetPath: '',
  cabinet: null,
  cabinetMissing: false,
  pickerPath: ''
};

var COVER_LEVELS = ['0 - DontCover', '1 - HigherCover'];
var SAVE_LEVELS = ['0 - DontSave', '1 - SaveHigher', '2 - SaveHigherEqual', '3 - SaveAll'];

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

function $(sel, root) { return (root || document).querySelector(sel); }
function $$(sel, root) { return Array.prototype.slice.call((root || document).querySelectorAll(sel)); }

function esc(value) {
  return String(value === undefined || value === null ? '' : value).replace(/[&<>"']/g, function (ch) {
    if (ch === '&') { return '&amp;'; }
    if (ch === '<') { return '&lt;'; }
    if (ch === '>') { return '&gt;'; }
    if (ch === '"') { return '&quot;'; }
    return '&#39;';
  });
}

async function api(path, options) {
  var init = Object.assign({ headers: { 'Content-Type': 'application/json' } }, options || {});
  var res = await fetch(path, init);
  var text = await res.text();
  var data = null;
  if (text) {
    try { data = JSON.parse(text); } catch (err) { data = { error: text }; }
  }
  if (!res.ok) { throw new Error((data && data.error) || (res.status + ' ' + res.statusText)); }
  return data;
}

function toast(title, message, kind) {
  var el = document.createElement('div');
  el.className = 'toast' + (kind ? ' ' + kind : '');
  el.innerHTML = '<div class="title">' + esc(title) + '</div>' +
    (message ? '<div class="muted small wrap">' + esc(message) + '</div>' : '');
  $('#toasts').appendChild(el);
  setTimeout(function () { el.remove(); }, kind === 'error' ? 9000 : 4200);
}

function modal(opts) {
  closeModal();
  var root = document.createElement('div');
  root.className = 'modal-backdrop';
  root.innerHTML =
    '<div class="modal">' +
      '<header><h3>' + esc(opts.title) + '</h3><button class="close" data-close>&times;</button></header>' +
      '<div class="body">' + (opts.body || '') + '</div>' +
      '<footer>' + (opts.footer || '<button class="btn" data-close>Close</button>') + '</footer>' +
    '</div>';
  root.addEventListener('click', function (ev) {
    if (ev.target === root || (ev.target.hasAttribute && ev.target.hasAttribute('data-close'))) { closeModal(); }
  });
  $('#modal-root').appendChild(root);
  if (opts.onMount) { opts.onMount(root); }
  return root;
}

function closeModal() { $('#modal-root').innerHTML = ''; }

function busy(btn, on) {
  if (!btn) { return; }
  btn.disabled = !!on;
  if (on) {
    btn.dataset.label = btn.textContent;
    btn.innerHTML = '<span class="spin"></span>';
  } else if (btn.dataset.label) {
    btn.textContent = btn.dataset.label;
    delete btn.dataset.label;
  }
}

function badge(text, kind) { return '<span class="badge' + (kind ? ' ' + kind : '') + '">' + esc(text) + '</span>'; }
function stat(value, label) {
  return '<div class="stat"><div class="value">' + esc(value) + '</div><div class="label">' + esc(label) + '</div></div>';
}
function flag(on) { return on ? badge('on', 'ok') : badge('off'); }

function fileListHtml(list, emptyText) {
  if (!list || !list.total) { return '<div class="muted small">' + esc(emptyText) + '</div>'; }
  var shown = list.files.slice(0, 12).map(function (f) { return esc(f); }).join('<br>');
  var more = list.total > 12 ? '<div class="muted">... ' + (list.total - 12) + ' more</div>' : '';
  return '<div class="filelist">' + shown + more + '</div>';
}

function findGroup(uuid) {
  var groups = (state.overview && state.overview.groups) || [];
  for (var i = 0; i < groups.length; i++) { if (groups[i].uuid === uuid) { return groups[i]; } }
  return null;
}

// ---------------------------------------------------------------------------
// top level loading
// ---------------------------------------------------------------------------

async function loadOverview(notify) {
  try {
    var data = await api('/api/overview');
    state.overview = data;
    state.config = data.config;
    renderTop();
    renderActive();
    if (notify) {
      toast('Rescan done', data.storages.length + ' storage(s), ' + data.groups.length + ' group(s)');
    }
  } catch (err) {
    toast('Scan failed', err.message, 'error');
  }
}

async function loadConfig() {
  state.config = await api('/api/config');
  if (state.tab === 'settings') { renderActive(); }
}

function renderTop() {
  var ov = state.overview;
  if (!ov) { return; }
  $('#version').textContent = 'rsdish ' + ov.version + ' - ' + ov.storages.length + ' storages, ' + ov.groups.length + ' groups';
  var conf = $('#config-path');
  conf.textContent = 'config: ' + ov.config.config_path;
  conf.title = ov.config.config_path;
}

function renderActive() {
  if (state.tab === 'dashboard') { renderDashboard(); }
  else if (state.tab === 'groups') { renderGroups(); }
  else if (state.tab === 'cabinets') { renderCabinets(); }
  else if (state.tab === 'settings') { renderSettings(); }
  else if (state.tab === 'logs') { renderLogs(); }
}

function switchTab(name) {
  state.tab = name;
  $$('#tabs .tab').forEach(function (tab) {
    tab.classList.toggle('active', tab.dataset.tab === name);
  });
  $$('main .panel').forEach(function (panel) {
    panel.classList.toggle('active', panel.id === 'panel-' + name);
  });
  if (name === 'logs') { startLogPolling(); } else { stopLogPolling(); }
  if (name === 'settings') { loadConfig().catch(function (err) { toast('Config failed', err.message, 'error'); }); }
  renderActive();
}

// ---------------------------------------------------------------------------
// dashboard
// ---------------------------------------------------------------------------

function renderDashboard() {
  var panel = $('#panel-dashboard');
  var ov = state.overview;

  if (!ov) { panel.innerHTML = '<div class="empty">Scanning storages...</div>'; return; }

  var cabinetCount = ov.storages.reduce(function (sum, stg) { return sum + stg.cabinet_count; }, 0);

  var html = '<div class="grid">' +
    stat(ov.storages.length, 'storages') +
    stat(cabinetCount, 'cabinets') +
    stat(ov.groups.length, 'groups') +
    stat(ov.issues.length, 'scan issues') +
    '</div>';

  if (ov.issues.length) {
    html += '<div class="card"><header><h3>Scan issues</h3><span class="muted small">a broken cabinet config never hides the others</span></header>' +
      '<table><thead><tr><th>path</th><th>message</th></tr></thead><tbody>';
    ov.issues.forEach(function (issue) {
      html += '<tr><td class="path wrap">' + esc(issue.path) + '</td><td class="wrap">' + esc(issue.message) + '</td></tr>';
    });
    html += '</tbody></table></div>';
  }

  html += '<div class="card"><header><h3>Storages</h3><span class="muted small">scanned: ' +
    ov.scan_paths.map(esc).join(' - ') + '</span></header>';

  if (!ov.storages.length) {
    html += '<div class="empty">No storage found. Add a custom storage in the Settings tab.</div>';
  }

  ov.storages.forEach(function (stg) {
    html += '<div class="card" style="background:var(--bg-soft)">' +
      '<header><h3 class="path">' + esc(stg.abs_path) + '</h3>' +
      badge(stg.from_disk_list ? 'mounted disk' : 'custom storage') +
      badge(stg.cabinet_count + ' cabinet(s)') + '</header><ul class="tree">';

    stg.cabinets.forEach(function (cab) {
      html += '<li><div class="row wrap-row"><span class="path">' + esc(cab.rel_path) + '</span>' +
        '<span class="spacer"></span>' +
        '<button class="btn tiny" data-edit-cabinet="' + esc(cab.abs_path) + '">Edit</button></div>';

      if (!cab.memberships.length) {
        html += '<div class="cabinet muted small">no membership yet</div>';
      } else {
        cab.memberships.forEach(function (mem) {
          var role = roleOf(mem);
          html += '<div class="cabinet row wrap-row">' +
            badge(role, role) +
            '<span class="mono small">' + esc(mem.group_uuid) + '</span>' +
            '<span class="muted small">p' + esc(mem.priority) + '</span>' +
            '<span class="muted small">src ' + (mem.src_option.enable ? 'on' : 'off') +
            ' dst ' + (mem.dst_option.enable ? 'on' : 'off') +
            ' link ' + (mem.link_option.enable ? 'on' : 'off') + '</span>' +
            '</div>';
        });
      }

      html += '</li>';
    });

    html += '</ul></div>';
  });

  html += '</div>';
  panel.innerHTML = html;
}

function roleOf(mem) {
  var src = mem.src_option.enable;
  var dst = mem.dst_option.enable;
  var link = mem.link_option.enable;
  if (src && dst && !link) { return 'main'; }
  if (src && !dst && !link) { return 'src'; }
  if (!src && dst && !link) { return 'dst'; }
  if (!src && !dst && link) { return 'gate'; }
  if (!src && !dst && !link) { return 'idle'; }
  return 'mixed';
}

// ---------------------------------------------------------------------------
// groups
// ---------------------------------------------------------------------------

function renderGroups() {
  var panel = $('#panel-groups');
  var ov = state.overview;

  if (!ov) { panel.innerHTML = '<div class="empty">Scanning groups...</div>'; return; }
  if (!ov.groups.length) {
    panel.innerHTML = '<div class="empty">No group found. A group exists once at least one cabinet declares a member.</div>';
    return;
  }

  var html = '';
  ov.groups.forEach(function (group) {
    html += '<div class="card" data-group="' + esc(group.uuid) + '">' +
      '<header><h3 class="mono">' + esc(group.uuid) + '</h3>' +
      badge('max priority ' + group.max_priority) +
      badge(group.members.length + ' member(s)') +
      '<span class="spacer"></span>' +
      '<div class="row">' +
        '<button class="btn tiny" data-act="plan" data-uuid="' + esc(group.uuid) + '">Preview</button>' +
        '<button class="btn tiny primary" data-act="sync" data-uuid="' + esc(group.uuid) + '">Sync</button>' +
        '<button class="btn tiny" data-act="link" data-uuid="' + esc(group.uuid) + '">Link</button>' +
        '<button class="btn tiny danger" data-act="clean" data-uuid="' + esc(group.uuid) + '">Clean</button>' +
        '<button class="btn tiny" data-act="exec" data-uuid="' + esc(group.uuid) + '">Exec</button>' +
      '</div></header>' +
      '<table><thead><tr><th>role</th><th>priority</th><th>src</th><th>dst</th><th>link</th><th>cabinet</th></tr></thead><tbody>';

    group.members.forEach(function (mem) {
      html += '<tr>' +
        '<td>' + badge(mem.role, mem.role) + '</td>' +
        '<td>' + esc(mem.priority) + '</td>' +
        '<td>' + flag(mem.src_enabled) + '</td>' +
        '<td>' + flag(mem.dst_enabled) + '</td>' +
        '<td>' + flag(mem.link_enabled) + '</td>' +
        '<td class="path wrap">' + esc(mem.cabinet_abs_path) + '<div class="muted small">on ' + esc(mem.storage_abs_path) + '</div></td>' +
        '</tr>';
    });

    html += '</tbody></table><div class="result" data-result="' + esc(group.uuid) + '"></div></div>';
  });

  panel.innerHTML = html;
}

function planHtml(action, plan) {
  var html = '<div class="row wrap-row" style="margin-bottom:10px">' +
    badge(plan.files.total + ' file(s) in virtual member') +
    badge('highest priority ' + plan.highest_priority) +
    (plan.strict ? badge('strict', 'warn') : badge('incremental')) +
    '</div>';

  html += '<table><thead><tr><th>role</th><th>p</th><th>cabinet</th><th>' +
    (action === 'sync' ? 'copies' : action === 'link' ? 'links' : action === 'clean' ? 'removals' : 'copies / links / removals') +
    '</th></tr></thead><tbody>';

  plan.members.forEach(function (mem) {
    var counts = action === 'sync' ? mem.copies.total
      : action === 'link' ? mem.links.total
      : action === 'clean' ? mem.cleans.total
      : mem.copies.total + ' / ' + mem.links.total + ' / ' + mem.cleans.total;

    html += '<tr>' +
      '<td>' + badge(mem.role, mem.role) + '</td>' +
      '<td>' + esc(mem.priority) + '</td>' +
      '<td class="path wrap">' + esc(mem.cabinet_abs_path) + '</td>' +
      '<td>' + esc(counts) + '</td>' +
      '</tr>';
  });

  html += '</tbody></table>';

  if (action === 'sync' || action === 'clean' || action === 'plan') {
    plan.members.forEach(function (mem) {
      var list = action === 'clean' ? mem.cleans : mem.copies;
      if (!list.total) { return; }
      html += '<h4 style="margin-top:14px">' + esc(mem.cabinet_abs_path) + ' - ' +
        (action === 'clean' ? 'to remove' : 'to copy') + '</h4>' + fileListHtml(list, 'nothing to do');
    });
  } else if (action === 'link') {
    plan.members.forEach(function (mem) {
      if (!mem.links.total) { return; }
      html += '<h4 style="margin-top:14px">' + esc(mem.cabinet_abs_path) + ' - to link</h4>' + fileListHtml(mem.links, 'nothing to do');
    });
  }

  return html;
}

async function openPlanModal(action, uuid) {
  var strict = false;
  var isSync = action === 'sync';
  var target = isSync ? 'sync' : action;

  var root = modal({
    title: (action === 'plan' ? 'Preview' : 'Confirm ' + action) + ' - ' + uuid,
    body: (isSync ? '<div class="row" style="margin-bottom:12px"><label class="row small"><input type="checkbox" id="strict-box">&nbsp;strict mode (copy every file, ignore size and time)</label></div>' : '') +
      '<div id="plan-area" class="muted small">Loading plan...</div>',
    footer: '<button class="btn" data-close>Cancel</button>' +
      (action === 'plan' ? '' : '<button class="btn primary" id="run-btn" disabled>Run ' + esc(action) + '</button>'),
    onMount: function (root) {
      var area = $('#plan-area', root);
      var runBtn = $('#run-btn', root);
      var strictBox = $('#strict-box', root);

      async function refresh() {
        area.innerHTML = '<span class="spin"></span> reading cabinets...';
        try {
          var plan = await api('/api/groups/' + encodeURIComponent(uuid) + '/plan?strict=' + (strict ? 'true' : 'false'));
          area.innerHTML = planHtml(action === 'plan' ? 'plan' : action, plan);
          if (runBtn) {
            var total = action === 'sync' ? plan.members.reduce(function (s, m) { return s + m.copies.total; }, 0)
              : action === 'link' ? plan.members.reduce(function (s, m) { return s + m.links.total; }, 0)
              : plan.members.reduce(function (s, m) { return s + m.cleans.total; }, 0);
            runBtn.disabled = total === 0;
            runBtn.textContent = total === 0 ? 'Nothing to do' : 'Run ' + action + ' (' + total + ')';
          }
        } catch (err) {
          area.innerHTML = '<span class="badge error">' + esc(err.message) + '</span>';
        }
      }

      if (strictBox) {
        strictBox.addEventListener('change', function () { strict = strictBox.checked; refresh(); });
      }

      if (runBtn) {
        runBtn.addEventListener('click', async function () {
          busy(runBtn, true);
          try {
            var body = isSync ? JSON.stringify({ strict: strict }) : JSON.stringify({});
            var result = await api('/api/groups/' + encodeURIComponent(uuid) + '/' + target, { method: 'POST', body: body });
            closeModal();
            showActionResult(action, uuid, result);
            loadOverview(false);
          } catch (err) {
            toast(action + ' failed', err.message, 'error');
          } finally {
            busy(runBtn, false);
          }
        });
      }

      refresh();
    }
  });

  return root;
}

function showActionResult(action, uuid, result) {
  var ok = result.failed === 0;
  var html = '<div class="row wrap-row" style="margin-bottom:10px">' +
    badge(result.planned + ' planned', ok ? 'ok' : 'warn') +
    badge(result.done + ' done', 'ok') +
    (result.failed ? badge(result.failed + ' failed', 'error') : badge('no failure', 'ok')) +
    '</div><table><thead><tr><th>cabinet</th><th>planned</th><th>done</th><th>failures</th></tr></thead><tbody>';

  result.members.forEach(function (mem) {
    html += '<tr><td class="path wrap">' + esc(mem.cabinet_abs_path) + '</td><td>' + esc(mem.planned) + '</td><td>' + esc(mem.done) + '</td><td>' +
      (mem.failures.length ? '<span class="badge error">' + mem.failures.length + '</span>' : '') + '</td></tr>';
    mem.failures.forEach(function (failure) {
      html += '<tr><td colspan="4" class="muted small wrap">' + esc(failure) + '</td></tr>';
    });
  });

  html += '</tbody></table>';

  modal({
    title: action + ' finished - ' + uuid,
    body: html,
    footer: '<button class="btn" data-close>Close</button><button class="btn primary" id="go-logs">See logs</button>',
    onMount: function (root) {
      $('#go-logs', root).addEventListener('click', function () { closeModal(); switchTab('logs'); });
    }
  });

  toast(action + ' ' + (ok ? 'finished' : 'finished with failures'), result.done + ' of ' + result.planned + ' done', ok ? '' : 'warn');
}

function openExecModal(uuid) {
  modal({
    title: 'Run script in every member of ' + uuid,
    body: '<label class="field">shell script, executed with the cabinet root as working directory</label>' +
      '<textarea id="script-box" placeholder="du -sh ."></textarea>' +
      '<div class="muted small" style="margin-top:8px">On Linux and macOS this runs through sh -c, on Windows through cmd /C.</div>',
    footer: '<button class="btn" data-close>Cancel</button><button class="btn primary" id="exec-run">Run</button>',
    onMount: function (root) {
      var box = $('#script-box', root);
      var runBtn = $('#exec-run', root);
      box.focus();

      runBtn.addEventListener('click', async function () {
        var script = box.value;
        if (!script.trim()) { toast('Script is empty', 'nothing to run', 'warn'); return; }
        busy(runBtn, true);
        try {
          var result = await api('/api/groups/' + encodeURIComponent(uuid) + '/exec', {
            method: 'POST',
            body: JSON.stringify({ script: script })
          });
          closeModal();
          showExecResult(uuid, result);
        } catch (err) {
          toast('exec failed', err.message, 'error');
        } finally {
          busy(runBtn, false);
        }
      });
    }
  });
}

function showExecResult(uuid, result) {
  var html = '';
  result.members.forEach(function (mem) {
    html += '<div class="card" style="margin-bottom:10px"><header>' +
      badge(mem.success ? 'exit ' + (mem.code === null ? 0 : mem.code) : 'failed', mem.success ? 'ok' : 'error') +
      '<span class="path wrap">' + esc(mem.cabinet_abs_path) + '</span></header>';
    if (mem.error) { html += '<div class="badge error">' + esc(mem.error) + '</div>'; }
    if (mem.stdout) { html += '<h4>stdout</h4><div class="filelist">' + esc(mem.stdout) + '</div>'; }
    if (mem.stderr) { html += '<h4>stderr</h4><div class="filelist">' + esc(mem.stderr) + '</div>'; }
    if (!mem.stdout && !mem.stderr && !mem.error) { html += '<div class="muted small">no output</div>'; }
    html += '</div>';
  });

  modal({
    title: 'exec finished - ' + uuid,
    body: html || '<div class="empty">no member to run in</div>',
    onMount: function () { }
  });
}

// ---------------------------------------------------------------------------
// cabinets
// ---------------------------------------------------------------------------

function renderCabinets() {
  var panel = $('#panel-cabinets');
  var html = '<div class="card"><header><h3>Cabinet</h3><span class="muted small">a directory holding rsdish.cabinet.toml</span></header>' +
    '<div class="form-row">' +
      '<div class="grow"><label class="field">cabinet directory</label>' +
      '<input type="text" id="cabinet-path" class="mono" value="' + esc(state.cabinetPath) + '" placeholder="/mnt/ssd/anime"></div>' +
      '<button class="btn" id="cabinet-open">Open</button>' +
      '<button class="btn" id="cabinet-choose">Choose...</button>' +
    '</div></div>';

  html += '<div id="cabinet-editor"></div>';
  panel.innerHTML = html;

  $('#cabinet-open').addEventListener('click', function () {
    state.cabinetPath = $('#cabinet-path').value.trim();
    loadCabinet();
  });

  $('#cabinet-choose').addEventListener('click', function () {
    openPicker(state.cabinetPath, function (picked) {
      state.cabinetPath = picked;
      $('#cabinet-path').value = picked;
      loadCabinet();
    });
  });

  $('#cabinet-path').addEventListener('keydown', function (ev) {
    if (ev.key === 'Enter') { state.cabinetPath = ev.target.value.trim(); loadCabinet(); }
  });

  if (state.cabinetPath) { loadCabinet(); } else { renderCabinetEditor(); }
}

async function loadCabinet() {
  var path = state.cabinetPath;
  if (!path) { state.cabinet = null; state.cabinetMissing = true; renderCabinetEditor(); return; }

  try {
    var data = await api('/api/cabinets?path=' + encodeURIComponent(path));
    state.cabinet = data.cabinet || null;
    state.cabinetMissing = !data.has_config;
    state.cabinetError = data.error || null;
  } catch (err) {
    state.cabinet = null;
    state.cabinetMissing = true;
    state.cabinetError = err.message;
  }

  renderCabinetEditor();
}

function renderCabinetEditor() {
  var host = $('#cabinet-editor');
  if (!host) { return; }

  if (!state.cabinetPath) {
    host.innerHTML = '<div class="empty">Pick a cabinet directory to edit its memberships.</div>';
    return;
  }

  var html = '<div class="card"><header><h3 class="path">' + esc(state.cabinetPath) + '</h3>';

  if (state.cabinetMissing) {
    html += badge('no rsdish.cabinet.toml', 'warn') + '</header>';
    if (state.cabinetError) { html += '<div class="muted small wrap">' + esc(state.cabinetError) + '</div>'; }
    html += '<div class="row wrap-row">' +
      '<button class="btn primary" data-cab-op="init">Init empty config</button>' +
      '<button class="btn" data-cab-op="join">Join a new group</button>' +
      '</div></div>';
    host.innerHTML = html;
    return;
  }

  var cab = state.cabinet;
  if (!cab) { host.innerHTML = '<div class="empty">Loading cabinet...</div>'; return; }

  html += badge(cab.memberships.length + ' membership(s)') + '</header>';
  html += '<div class="row wrap-row" style="margin-bottom:12px">' +
    '<button class="btn tiny" data-cab-op="preset" data-preset="main">Preset main</button>' +
    '<button class="btn tiny" data-cab-op="preset" data-preset="mirror">Preset mirror</button>' +
    '<button class="btn tiny" data-cab-op="preset" data-preset="gate">Preset gate</button>' +
    '<span class="spacer"></span>' +
    '<button class="btn tiny" data-cab-op="join">Join group / add membership</button>' +
    '<button class="btn tiny primary" data-cab-op="save">Save memberships</button>' +
    '</div>';

  html += '<table><thead><tr><th>group uuid</th><th>priority</th><th>src</th><th>dst (cover / save)</th><th>link (save)</th><th></th></tr></thead><tbody>';

  cab.memberships.forEach(function (mem, index) {
    html += '<tr data-row="' + index + '">' +
      '<td><input type="text" class="mono" data-field="group_uuid" value="' + esc(mem.group_uuid) + '"></td>' +
      '<td><input type="number" data-field="priority" value="' + esc(mem.priority) + '" style="width:78px"></td>' +
      '<td><input type="checkbox" data-field="src_enable"' + (mem.src_option.enable ? ' checked' : '') + '></td>' +
      '<td><input type="checkbox" data-field="dst_enable"' + (mem.dst_option.enable ? ' checked' : '') + '>' +
        '<select data-field="cover_level" style="margin-top:6px">' + optionsHtml(COVER_LEVELS, mem.dst_option.cover_level) + '</select>' +
        '<select data-field="save_level" style="margin-top:6px">' + optionsHtml(SAVE_LEVELS, mem.dst_option.save_level) + '</select></td>' +
      '<td><input type="checkbox" data-field="link_enable"' + (mem.link_option.enable ? ' checked' : '') + '>' +
        '<select data-field="link_save_level" style="margin-top:6px">' + optionsHtml(SAVE_LEVELS, mem.link_option.save_level) + '</select></td>' +
      '<td class="actions"><button class="btn tiny danger" data-cab-op="remove" data-index="' + index + '">Remove</button></td>' +
      '</tr>';
  });

  html += '</tbody></table>';
  html += '<div class="muted small" style="margin-top:10px">src feeds the virtual member, dst receives copies, link receives symlinks. Levels: 0 dont save, 1 save higher, 2 save higher or equal, 3 save all.</div>';
  html += '</div>';

  host.innerHTML = html;
}

function optionsHtml(labels, value) {
  var html = '';
  for (var i = 0; i < labels.length; i++) {
    html += '<option value="' + i + '"' + (Number(value) === i ? ' selected' : '') + '>' + esc(labels[i]) + '</option>';
  }
  return html;
}

function collectMemberships() {
  var rows = $$('#cabinet-editor tr[data-row]');
  var memberships = [];

  rows.forEach(function (row) {
    function field(name) { return $('[data-field="' + name + '"]', row); }
    memberships.push({
      group_uuid: field('group_uuid').value.trim(),
      priority: Number(field('priority').value) || 0,
      src_option: { enable: field('src_enable').checked },
      dst_option: {
        enable: field('dst_enable').checked,
        cover_level: Number(field('cover_level').value) || 0,
        save_level: Number(field('save_level').value) || 0
      },
      link_option: {
        enable: field('link_enable').checked,
        save_level: Number(field('link_save_level').value) || 0
      }
    });
  });

  return memberships;
}

async function cabinetOperation(op, extra) {
  var path = state.cabinetPath;
  var payload = Object.assign({ path: path }, extra || {});
  var endpoint = '/api/cabinets/' + op;
  var method = op === 'save' ? 'PUT' : 'POST';
  if (op === 'save') { endpoint = '/api/cabinets/config'; }

  var data = await api(endpoint, { method: method, body: JSON.stringify(payload) });
  if (data.cabinet) { state.cabinet = data.cabinet; state.cabinetMissing = false; }
  toast('Cabinet updated', data.message || path);
  renderCabinetEditor();
  loadOverview(false);
}

async function onCabinetClick(ev) {
  var btn = ev.target.closest ? ev.target.closest('[data-cab-op]') : null;
  if (!btn) { return; }
  var op = btn.dataset.cabOp;

  try {
    if (op === 'init') {
      await cabinetOperation('init', {});
    } else if (op === 'preset') {
      await cabinetOperation('preset', { preset: btn.dataset.preset });
    } else if (op === 'join') {
      askJoin(btn);
    } else if (op === 'save') {
      busy(btn, true);
      await cabinetOperation('save', { memberships: collectMemberships() });
    } else if (op === 'remove') {
      var index = Number(btn.dataset.index);
      var memberships = state.cabinet.memberships.filter(function (_, i) { return i !== index; });
      busy(btn, true);
      await cabinetOperation('save', { memberships: memberships });
    }
  } catch (err) {
    toast('Cabinet operation failed', err.message, 'error');
  } finally {
    busy(btn, false);
  }
}

function askJoin(btn) {
  modal({
    title: 'Join group',
    body: '<label class="field">paste one uuid per line, or leave empty to join a brand new group</label>' +
      '<textarea id="join-box" placeholder="0199ebad-44ad-78a2-baad-c56a052e33ac"></textarea>',
    footer: '<button class="btn" data-close>Cancel</button><button class="btn primary" id="join-run">Join</button>',
    onMount: function (root) {
      $('#join-run', root).addEventListener('click', async function () {
        var raw = $('#join-box', root).value;
        var uuids = raw.split(/[^0-9a-zA-Z-]+/).filter(function (v) { return v.length > 0; });
        closeModal();
        try {
          await cabinetOperation('join', { group_uuids: uuids });
        } catch (err) {
          toast('Join failed', err.message, 'error');
        }
      });
    }
  });
}

// ---------------------------------------------------------------------------
// settings
// ---------------------------------------------------------------------------

function renderSettings() {
  var panel = $('#panel-settings');
  var conf = state.config;

  if (!conf) { panel.innerHTML = '<div class="empty">Loading config...</div>'; return; }

  var html = '<div class="card"><header><h3>Config file</h3></header>' +
    '<div class="path wrap">' + esc(conf.config_path) + '</div>' +
    '<div class="muted small" style="margin-top:6px">edit it with a text editor, or use the list below</div></div>';

  html += '<div class="card"><header><h3>Custom storages</h3><span class="muted small">extra roots scanned in addition to mounted disks</span></header>';
  html += '<div id="storage-rows">';

  conf.custom_storages.forEach(function (path, index) {
    html += '<div class="form-row" data-storage-row="' + index + '">' +
      '<div class="grow"><input type="text" class="mono" data-storage-path value="' + esc(path) + '"></div>' +
      '<button class="btn" data-storage-pick="' + index + '">Choose...</button>' +
      '<button class="btn danger" data-storage-remove="' + index + '">Remove</button>' +
      '</div>';
  });

  html += '</div>' +
    '<div class="row wrap-row" style="margin-top:10px">' +
    '<button class="btn" id="storage-add">Add path</button>' +
    '<span class="spacer"></span>' +
    '<button class="btn primary" id="storage-save">Save custom storages</button>' +
    '</div>';

  if (conf.warnings.length) {
    html += '<div style="margin-top:12px">';
    conf.warnings.forEach(function (warning) { html += '<div class="badge warn" style="margin:3px 4px 0 0">' + esc(warning) + '</div>'; });
    html += '</div>';
  }

  html += '</div>';
  panel.innerHTML = html;

  $('#storage-add').addEventListener('click', function () {
    var rows = $('#storage-rows');
    var index = rows.querySelectorAll('[data-storage-row]').length;
    var div = document.createElement('div');
    div.className = 'form-row';
    div.setAttribute('data-storage-row', String(index));
    div.innerHTML = '<div class="grow"><input type="text" class="mono" data-storage-path value=""></div>' +
      '<button class="btn" data-storage-pick="' + index + '">Choose...</button>' +
      '<button class="btn danger" data-storage-remove="' + index + '">Remove</button>';
    rows.appendChild(div);
  });

  $('#storage-save').addEventListener('click', async function (ev) {
    var btn = ev.currentTarget;
    var paths = $$('#storage-rows [data-storage-path]').map(function (input) { return input.value.trim(); })
      .filter(function (v) { return v.length > 0; });
    busy(btn, true);
    try {
      state.config = await api('/api/config', { method: 'PUT', body: JSON.stringify({ custom_storages: paths }) });
      toast('Settings saved', paths.length + ' custom storage(s)');
      renderSettings();
      loadOverview(false);
    } catch (err) {
      toast('Save failed', err.message, 'error');
    } finally {
      busy(btn, false);
    }
  });
}

// ---------------------------------------------------------------------------
// directory picker
// ---------------------------------------------------------------------------

function openPicker(startPath, onPick) {
  state.pickerPath = startPath || '';

  var root = modal({
    title: 'Choose a directory',
    body: '<div class="form-row"><div class="grow"><input type="text" id="picker-path" class="mono" value="' + esc(state.pickerPath) + '"></div>' +
      '<button class="btn" id="picker-go">Go</button><button class="btn" id="picker-up">Up</button></div>' +
      '<div id="picker-list" class="browse-list"><div class="empty">Loading...</div></div>',
    footer: '<button class="btn" data-close>Cancel</button><button class="btn primary" id="picker-pick">Use this directory</button>',
    onMount: function (root) {
      async function load(path) {
        var list = $('#picker-list', root);
        list.innerHTML = '<div class="empty">Loading...</div>';
        try {
          var data = await api('/api/browse?path=' + encodeURIComponent(path || ''));
          state.pickerPath = data.path;
          $('#picker-path', root).value = data.path;

          var html = '';
          data.entries.forEach(function (entry) {
            html += '<div class="browse-item" data-dir="' + esc(entry.path) + '">' +
              '<span class="path">' + esc(entry.name) + '</span>' +
              (entry.has_cabinet_config ? badge('cabinet', 'main') : '') +
              (entry.group_uuids.length ? '<span class="muted small mono">' + esc(entry.group_uuids.join(', ')) + '</span>' : '') +
              '<span class="spacer"></span><span class="muted small">open</span></div>';
          });

          list.innerHTML = html || '<div class="empty">no sub directory</div>';
          if (data.truncated) { list.innerHTML += '<div class="empty">listing truncated</div>'; }

          $$('.browse-item', list).forEach(function (item) {
            item.addEventListener('click', function () { load(item.dataset.dir); });
          });
        } catch (err) {
          list.innerHTML = '<div class="empty">' + esc(err.message) + '</div>';
        }
      }

      $('#picker-go', root).addEventListener('click', function () { load($('#picker-path', root).value); });
      $('#picker-up', root).addEventListener('click', function () { load(state.pickerPath ? state.pickerPath.replace(/[^/]+$/, '') : ''); });
      $('#picker-pick', root).addEventListener('click', function () {
        var picked = state.pickerPath;
        closeModal();
        onPick(picked);
      });
      $('#picker-path', root).addEventListener('keydown', function (ev) {
        if (ev.key === 'Enter') { load(ev.target.value); }
      });

      load(state.pickerPath);
    }
  });

  return root;
}

// ---------------------------------------------------------------------------
// logs
// ---------------------------------------------------------------------------

function startLogPolling() {
  if (state.logTimer) { return; }
  state.logTimer = setInterval(pollLogs, 1200);
  pollLogs();
}

function stopLogPolling() {
  if (state.logTimer) { clearInterval(state.logTimer); state.logTimer = null; }
}

async function pollLogs() {
  try {
    var data = await api('/api/logs?since=' + state.logCursor + '&limit=400');
    if (data.lines.length) {
      state.logLines = state.logLines.concat(data.lines);
      if (state.logLines.length > 3000) { state.logLines = state.logLines.slice(state.logLines.length - 3000); }
    }
    state.logCursor = data.next;
    if (state.tab === 'logs') { renderLogs(); }
  } catch (err) {
    // keep the UI quiet, the next tick will retry
  }
}

function renderLogs() {
  var panel = $('#panel-logs');

  if (!state.logLines.length) {
    panel.innerHTML = '<div class="log-toolbar">' +
      '<select id="log-level"><option value="ALL">all levels</option><option value="INFO">info+</option><option value="WARN">warn+</option><option value="ERROR">error only</option></select>' +
      '<label class="row small"><input type="checkbox" id="log-follow"' + (state.logFollow ? ' checked' : '') + '>&nbsp;follow</label>' +
      '<button class="btn tiny" id="log-clear">Clear view</button>' +
      '<span class="spacer"></span><span class="muted small">waiting for events...</span></div>' +
      '<div class="log-view" id="log-view"><div class="empty">no log line yet</div></div>';
    wireLogToolbar(panel);
    return;
  }

  var minLevel = state.logLevel;
  var order = { TRACE: 0, DEBUG: 1, INFO: 2, WARN: 3, ERROR: 4 };

  var html = '<div class="log-toolbar">' +
    '<select id="log-level">' +
      '<option value="ALL"' + (minLevel === 'ALL' ? ' selected' : '') + '>all levels</option>' +
      '<option value="INFO"' + (minLevel === 'INFO' ? ' selected' : '') + '>info+</option>' +
      '<option value="WARN"' + (minLevel === 'WARN' ? ' selected' : '') + '>warn+</option>' +
      '<option value="ERROR"' + (minLevel === 'ERROR' ? ' selected' : '') + '>error only</option>' +
    '</select>' +
    '<label class="row small"><input type="checkbox" id="log-follow"' + (state.logFollow ? ' checked' : '') + '>&nbsp;follow</label>' +
    '<button class="btn tiny" id="log-clear">Clear view</button>' +
    '<span class="spacer"></span><span class="muted small">' + state.logLines.length + ' line(s) buffered</span>' +
    '</div><div class="log-view" id="log-view">';

  state.logLines.forEach(function (line) {
    if (minLevel !== 'ALL' && order[line.level] < order[minLevel]) { return; }
    html += '<div class="log-line ' + esc(line.level) + '">' +
      '<span class="seq">' + esc(line.seq) + '</span>' +
      '<span class="lvl">' + esc(line.level) + '</span>' +
      '<span class="target">' + esc(line.target) + '</span>' +
      '<span class="msg">' + esc(line.message) + '</span></div>';
  });

  html += '</div>';
  panel.innerHTML = html;
  wireLogToolbar(panel);

  if (state.logFollow) {
    var view = $('#log-view', panel);
    if (view) { view.scrollTop = view.scrollHeight; }
  }
}

function wireLogToolbar(panel) {
  var level = $('#log-level', panel);
  var follow = $('#log-follow', panel);
  var clear = $('#log-clear', panel);

  if (level) { level.addEventListener('change', function () { state.logLevel = level.value; renderLogs(); }); }
  if (follow) { follow.addEventListener('change', function () { state.logFollow = follow.checked; }); }
  if (clear) {
    clear.addEventListener('click', function () {
      state.logLines = [];
      renderLogs();
    });
  }
}

// ---------------------------------------------------------------------------
// boot
// ---------------------------------------------------------------------------

function onGroupsClick(ev) {
  var btn = ev.target.closest ? ev.target.closest('[data-act]') : null;
  if (!btn) { return; }
  var action = btn.dataset.act;
  var uuid = btn.dataset.uuid;

  if (action === 'exec') { openExecModal(uuid); return; }
  openPlanModal(action, uuid);
}

function onSettingsClick(ev) {
  var pick = ev.target.closest ? ev.target.closest('[data-storage-pick]') : null;
  var remove = ev.target.closest ? ev.target.closest('[data-storage-remove]') : null;

  if (pick) {
    var row = pick.closest('[data-storage-row]');
    var input = $('[data-storage-path]', row);
    openPicker(input.value, function (picked) { input.value = picked; });
  } else if (remove) {
    remove.closest('[data-storage-row]').remove();
  }
}

function onDashboardClick(ev) {
  var btn = ev.target.closest ? ev.target.closest('[data-edit-cabinet]') : null;
  if (!btn) { return; }
  state.cabinetPath = btn.dataset.editCabinet;
  switchTab('cabinets');
}

function init() {
  $('#refresh-btn').addEventListener('click', function () { loadOverview(true); });

  $$('#tabs .tab').forEach(function (tab) {
    tab.addEventListener('click', function () { switchTab(tab.dataset.tab); });
  });

  $('#panel-groups').addEventListener('click', onGroupsClick);
  $('#panel-dashboard').addEventListener('click', onDashboardClick);
  $('#panel-cabinets').addEventListener('click', onCabinetClick);
  $('#panel-settings').addEventListener('click', onSettingsClick);

  loadOverview(false);
  loadConfig().catch(function () { });
}

document.addEventListener('DOMContentLoaded', init);
