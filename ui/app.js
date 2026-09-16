/* Local assets keep the desktop UI independent of third-party requests. */
(() => {
  'use strict';
  const byId = id => document.getElementById(id);
  const bridge = window.__TAURI__;
  let config;
  let saving = false;
  let searchTimer;
  let historyRequest = 0;
  let activePage = 'settings';
  const message = (id, text, error = false) => {
    byId(id).textContent = text;
    byId(id).classList.toggle('error', error);
  };
  const escapeHtml = text => String(text).replace(/[&<>"']/g, char => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[char]));

  function showStatus(payload) {
    const text = typeof payload === 'string' ? payload : payload.message;
    const transcribing = /transcrib/i.test(text);
    const listening = /listening/i.test(text);
    const active = typeof payload === 'object' ? payload.active : listening || transcribing;
    byId('status').textContent = text;
    byId('status-card').classList.toggle('is-active', Boolean(active));
    byId('status-hint').textContent = transcribing ? 'Turning voice into words' : listening ? 'Listening' : 'Ready for dictation';
  }

  function changePage(name, focus = false) {
    if (!['settings','history'].includes(name)) return;
    activePage = name;
    document.querySelectorAll('.page').forEach(page => {
      const selected = page.id === name;
      page.hidden = !selected;
      page.classList.toggle('active', selected);
    });
    document.querySelectorAll('nav [data-page]').forEach(button => {
      const selected = button.dataset.page === name;
      button.classList.toggle('active', selected);
      if (selected) button.setAttribute('aria-current','page'); else button.removeAttribute('aria-current');
    });
    document.querySelector('.skip-link').href = `#${name}-title`;
    window.scrollTo({top:0, behavior:'instant'});
    if (focus) byId(`${name}-title`).focus({preventScroll:true});
    if (name === 'history') loadHistory();
  }

  async function loadConfig() {
    try {
      config = await bridge.core.invoke('get_config');
      byId('key').value = config.transcription.groq_api_key;
      byId('model').value = config.transcription.model;
      byId('language').value = config.transcription.language;
      byId('handsfree').checked = config.shortcuts.handsfree_enabled;
      byId('autostart').checked = config.app.autostart;
      byId('save-history').checked = config.app.save_history;
      byId('push-keys').innerHTML = config.shortcuts.push_to_talk.split('+').map(key => `<kbd>${escapeHtml(key.trim().replace(/^./, c => c.toUpperCase()))}</kbd>`).join('<span>+</span>');
      byId('settings-fields').disabled = false;
      byId('save').disabled = false;
      message('settings-message', 'Changes are saved on this device.');
    } catch (error) { message('settings-message', `Could not load settings: ${error}`, true); }
  }

  async function saveConfig() {
    if (!config || saving) return;
    saving = true;
    byId('save').disabled = true;
    const updated = JSON.parse(JSON.stringify(config));
    updated.transcription.groq_api_key = byId('key').value.trim();
    updated.transcription.model = byId('model').value.trim() || 'whisper-large-v3-turbo';
    updated.transcription.language = byId('language').value.trim() || 'en';
    updated.shortcuts.handsfree_enabled = byId('handsfree').checked;
    updated.app.autostart = byId('autostart').checked;
    updated.app.save_history = byId('save-history').checked;
    byId('settings-fields').disabled = true;
    message('settings-message', 'Saving your preferences…');
    try {
      await bridge.core.invoke('save_config', {config:updated});
      config = updated;
      byId('model').value = updated.transcription.model;
      byId('language').value = updated.transcription.language;
      message('settings-message', 'Changes saved.');
    } catch (error) { message('settings-message', String(error), true); }
    finally { saving = false; byId('save').disabled = false; byId('settings-fields').disabled = false; }
  }

  function emptyState(searching, error = false) {
    const title = error ? 'History is unavailable.' : searching ? 'No words found.' : 'No saved dictations yet.';
    const text = error ? 'Try opening History again to reload your dictations.' : searching ? 'Try another word or clear your search.' : 'Enable “Save dictation history” in Settings to keep your next transcript here.';
    return `<div class="empty"><div class="empty-mark" aria-hidden="true">“</div><h2>${title}</h2><p>${text}</p></div>`;
  }

  async function loadHistory() {
    const request = ++historyRequest;
    const search = byId('search').value;
    if (!bridge) { byId('entries').innerHTML = emptyState(false); byId('clear-history').disabled = true; return; }
    message('history-message', 'Loading your words…');
    try {
      const entries = await bridge.core.invoke('history_list', {search});
      if (request !== historyRequest) return;
      byId('history-count').textContent = `${entries.length} ${entries.length === 1 ? 'dictation' : 'dictations'}`;
      byId('clear-history').disabled = entries.length === 0 && !search;
      byId('entries').innerHTML = entries.length ? entries.map(entry => {
        const date = new Date(entry.created_at);
        const dateText = Number.isNaN(date.getTime()) ? entry.created_at : date.toLocaleString(undefined, {dateStyle:'medium',timeStyle:'short'});
        const failed = entry.insertion_status === 'failed';
        const status = failed ? 'Paste failed' : entry.insertion_status === 'inserted' ? 'Pasted' : 'Pending paste';
        return `<article class="entry"><div class="entry-top"><time>${escapeHtml(dateText)}</time><div class="entry-actions"><button class="copy-entry" data-copy="${escapeHtml(entry.transcript)}" aria-label="Copy dictation from ${escapeHtml(dateText)}">Copy</button><button class="delete-entry" data-delete="${Number(entry.id)}" aria-label="Delete dictation from ${escapeHtml(dateText)}">Delete</button></div></div><p class="entry-text">${escapeHtml(entry.transcript)}</p><div class="meta"><span>${Number(entry.duration).toFixed(1)} sec</span><span>${escapeHtml(entry.provider)} / ${escapeHtml(entry.model)}</span><span class="outcome ${failed ? 'failed' : ''}">${status}</span></div></article>`;
      }).join('') : emptyState(Boolean(search.trim()));
      message('history-message', config?.app.save_history === false ? 'History saving is off. Existing dictations are still available.' : '');
    } catch (error) {
      if (request !== historyRequest) return;
      byId('entries').innerHTML = emptyState(false, true);
      byId('history-count').textContent = '';
      message('history-message', String(error), true);
    }
  }

  document.querySelectorAll('[data-page]').forEach(button => button.addEventListener('click', () => changePage(button.dataset.page, true)));
  document.querySelector('.brand').addEventListener('click', event => {event.preventDefault();changePage('settings', true);});
  byId('save').addEventListener('click', saveConfig);
  byId('settings-fields').addEventListener('input', () => message('settings-message', 'You have unsaved changes.'));
  byId('reveal-key').addEventListener('click', event => {
    const reveal = byId('key').type === 'password';
    byId('key').type = reveal ? 'text' : 'password';
    event.currentTarget.textContent = reveal ? 'Hide' : 'Show';
    event.currentTarget.setAttribute('aria-label', reveal ? 'Hide API key' : 'Show API key');
    event.currentTarget.setAttribute('aria-pressed', String(reveal));
  });
  byId('search').addEventListener('input', () => {
    ++historyRequest;
    clearTimeout(searchTimer);
    searchTimer = setTimeout(loadHistory, 160);
  });
  byId('entries').addEventListener('click', async event => {
    const copyButton = event.target.closest('[data-copy]');
    if (copyButton && bridge) {
      const original = copyButton.textContent;
      copyButton.disabled = true;
      try {
        await bridge.core.invoke('copy_to_clipboard', {text: copyButton.dataset.copy});
        copyButton.textContent = 'Copied';
        setTimeout(() => {copyButton.textContent = original; copyButton.disabled = false;}, 1400);
      } catch (error) {
        copyButton.disabled = false;
        message('history-message', String(error), true);
      }
      return;
    }
    const button = event.target.closest('[data-delete]');
    if (!button || !bridge) return;
    button.disabled = true;
    try { await bridge.core.invoke('history_delete', {id:Number(button.dataset.delete)}); await loadHistory(); }
    catch (error) {button.disabled = false; message('history-message', String(error), true);}
  });
  byId('clear-history').addEventListener('click', () => {
    byId('delete-dialog').returnValue = 'cancel';
    byId('delete-dialog').showModal();
  });
  byId('delete-dialog').addEventListener('close', async () => {
    if (byId('delete-dialog').returnValue !== 'delete' || !bridge) return;
    try {await bridge.core.invoke('history_clear'); await loadHistory();}
    catch (error) {message('history-message', String(error), true);}
  });
  if (bridge) {
    loadConfig();
    bridge.core.invoke('get_status').then(showStatus).catch(error => showStatus(`Status unavailable: ${error}`));
    bridge.event.listen('flow-status', ({payload}) => showStatus(payload));
    bridge.event.listen('flow-status-refresh', () => bridge.core.invoke('get_status').then(showStatus).catch(error => showStatus(`Status unavailable: ${error}`)));
    bridge.event.listen('history-updated', () => {if (activePage === 'history') loadHistory();});
  } else {
    showStatus('Desktop preview');
    message('settings-message', 'Open Flow to load and save your preferences.');
  }
})();
