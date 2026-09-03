// No external dependencies needed

type ToastType = 'info' | 'success' | 'warning' | 'error';

interface ToastOptions {
  message: string;
  type?: ToastType;
  duration?: number;
}

const toastContainer = (() => {
  let container = document.getElementById('dk-toast-container');
  if (!container) {
    container = document.createElement('div');
    container.id = 'dk-toast-container';
    container.style.cssText = `
      position:fixed;bottom:24px;right:24px;z-index:9999;
      display:flex;flex-direction:column;gap:8px;pointer-events:none;
    `;
    document.body.appendChild(container);
  }
  return container;
})();

export function toast(options: ToastOptions): void {
  const { message, type = 'info', duration = 3000 } = options;

  const colors: Record<ToastType, { bg: string; border: string }> = {
    info: { bg: 'var(--info-bg, #e3f2fd)', border: 'var(--info-border, #2196f3)' },
    success: { bg: 'var(--success-bg, #e8f5e9)', border: 'var(--success-border, #4caf50)' },
    warning: { bg: 'var(--warning-bg, #fff3e0)', border: 'var(--warning-border, #ff9800)' },
    error: { bg: 'var(--error-bg, #fbe9e7)', border: 'var(--error-border, #d32f2f)' },
  };

  const icons: Record<ToastType, string> = {
    info: 'ℹ️',
    success: '✅',
    warning: '⚠️',
    error: '❌',
  };

  const el = document.createElement('div');
  el.style.cssText = `
    background:${colors[type].bg};border-left:4px solid ${colors[type].border};
    padding:12px 16px;border-radius:8px;box-shadow:0 4px 12px rgba(0,0,0,0.15);
    display:flex;align-items:center;gap:8px;min-width:200px;max-width:400px;
    animation:slideIn 0.3s ease;pointer-events:auto;
  `;
  el.innerHTML = `<span>${icons[type]}</span><span style="flex:1;">${message}</span>`;

  // Add animation styles if not already added
  if (!document.getElementById('dk-toast-styles')) {
    const style = document.createElement('style');
    style.id = 'dk-toast-styles';
    style.textContent = `
      @keyframes slideIn {
        from { transform: translateX(100%); opacity: 0; }
        to { transform: translateX(0); opacity: 1; }
      }
      @keyframes slideOut {
        from { transform: translateX(0); opacity: 1; }
        to { transform: translateX(100%); opacity: 0; }
      }
    `;
    document.head.appendChild(style);
  }

  toastContainer.appendChild(el);

  setTimeout(() => {
    el.style.animation = 'slideOut 0.3s ease forwards';
    setTimeout(() => el.remove(), 300);
  }, duration);
}

export function toastSuccess(message: string, duration?: number): void {
  toast({ message, type: 'success', duration });
}

export function toastError(message: string, duration?: number): void {
  toast({ message, type: 'error', duration });
}

export function toastWarning(message: string, duration?: number): void {
  toast({ message, type: 'warning', duration });
}

export function toastInfo(message: string, duration?: number): void {
  toast({ message, type: 'info', duration });
}