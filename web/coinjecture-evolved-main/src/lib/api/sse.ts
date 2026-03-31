// SSE (Server-Sent Events) connection manager with auto-reconnect.

import { API_BASE } from './client';

interface SSEHandlers {
  onEvent: (eventType: string, data: any) => void;
  onError?: (error: Event) => void;
  onOpen?: () => void;
}

const EVENT_TYPES = ['block', 'mempool', 'new_order', 'trade', 'task_posted', 'connected'];

export function createSSEConnection(
  path: string,
  handlers: SSEHandlers,
): { close: () => void } {
  const url = `${API_BASE}${path}`;
  let eventSource: EventSource | null = null;
  let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  let closed = false;

  function connect() {
    if (closed) return;

    eventSource = new EventSource(url);

    eventSource.onopen = () => {
      handlers.onOpen?.();
    };

    eventSource.onerror = (e) => {
      handlers.onError?.(e);
      eventSource?.close();
      if (!closed) {
        reconnectTimer = setTimeout(connect, 3000);
      }
    };

    // Register listeners for known event types
    for (const type of EVENT_TYPES) {
      eventSource.addEventListener(type, (e: MessageEvent) => {
        try {
          const data = JSON.parse(e.data);
          handlers.onEvent(type, data);
        } catch {
          handlers.onEvent(type, e.data);
        }
      });
    }
  }

  connect();

  return {
    close: () => {
      closed = true;
      if (reconnectTimer) clearTimeout(reconnectTimer);
      eventSource?.close();
    },
  };
}
