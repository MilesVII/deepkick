import { loadHistory, clearHistory, type HistoryEntry } from '../persist';
import { toastSuccess } from '../components/toast';

export function historyUnit(): void {
  const container = document.getElementById('history-list');
  const clearBtn = document.getElementById('btn-clear-history');

  if (!container) return;

  renderHistory();

  clearBtn?.addEventListener('click', async () => {
    await clearHistory();
    renderHistory();
    toastSuccess('History cleared');
  });
}

async function renderHistory(): Promise<void> {
  const container = document.getElementById('history-list');
  if (!container) return;

  const history = await loadHistory();

  if (history.length === 0) {
    container.innerHTML = '<p style="text-align:center;color:var(--text-secondary,#666);padding:24px;">No processing history yet</p>';
    return;
  }

  container.innerHTML = history
    .slice()
    .reverse()
    .map(entry => createHistoryEntryHTML(entry))
    .join('');

  // Add click handlers for expandable entries
  container.querySelectorAll('.history-entry-header').forEach(header => {
    header.addEventListener('click', () => {
      const entry = header.closest('.history-entry');
      entry?.classList.toggle('expanded');
    });
  });
}

function createHistoryEntryHTML(entry: HistoryEntry): string {
  const date = new Date(entry.timestamp).toLocaleString();
  const { total, found, notFound, errors, skipped } = entry.stats;

  return `
    <div class="history-entry" style="border:1px solid var(--border,#e0e0e0);border-radius:8px;margin-bottom:12px;overflow:hidden;">
      <div class="history-entry-header" style="padding:16px;background:var(--surface-hover,#f5f5f5);cursor:pointer;display:flex;justify-content:space-between;align-items:center;">
        <div>
          <div style="font-weight:500;">Processing Session</div>
          <div style="font-size:12px;color:var(--text-secondary,#666);">${date}</div>
        </div>
        <div style="display:flex;gap:16px;font-size:12px;">
          <span style="color:var(--success,#4caf50);">✓ ${found}</span>
          <span style="color:var(--warning,#ff9800);">✗ ${notFound}</span>
          <span style="color:var(--error,#d32f2f);">⚠ ${errors}</span>
          <span style="color:var(--text-secondary,#666);">⊘ ${skipped}</span>
          <span>Total: ${total}</span>
        </div>
      </div>
      <div class="history-entry-details" style="padding:0 16px 16px;display:none;">
        <table style="width:100%;border-collapse:collapse;font-size:13px;">
          <thead>
            <tr style="text-align:left;border-bottom:1px solid var(--border,#e0e0e0);">
              <th style="padding:8px;">Track</th>
              <th style="padding:8px;">Artist - Title</th>
              <th style="padding:8px;">Status</th>
            </tr>
          </thead>
          <tbody>
            ${entry.tracks.map(t => `
              <tr style="border-bottom:1px solid var(--border,#e0e0e0);">
                <td style="padding:8px;">${t.name}</td>
                <td style="padding:8px;">${t.artist} - ${t.title}</td>
                <td style="padding:8px;">
                  <span style="padding:2px 8px;border-radius:4px;font-size:11px;${getStatusStyle(t.status)}">
                    ${t.status.replace('_', ' ')}
                  </span>
                </td>
              </tr>
            `).join('')}
          </tbody>
        </table>
      </div>
    </div>
  `;
}

function getStatusStyle(status: string): string {
  switch (status) {
    case 'found': return 'background:var(--success-bg,#e8f5e9);color:var(--success,#4caf50);';
    case 'not_found': return 'background:var(--warning-bg,#fff3e0);color:var(--warning,#ff9800);';
    case 'error': return 'background:var(--error-bg,#fbe9e7);color:var(--error,#d32f2f);';
    case 'skipped': return 'background:var(--surface-hover,#f5f5f5);color:var(--text-secondary,#666);';
    default: return '';
  }
}