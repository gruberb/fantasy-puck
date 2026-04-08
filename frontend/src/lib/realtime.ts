import { API_URL } from '@/config';
import { authService } from '@/features/auth';
import type { DraftSession, DraftPick } from '@/features/draft/types';

// ── Types ─────────────────────────────────────────────────────────────────

export interface DraftEventHandlers {
  onSessionUpdated?: (session: Partial<DraftSession>) => void;
  onPickMade?: (pick: DraftPick) => void;
  onSleeperUpdated?: () => void;
}

export interface RealtimeService {
  subscribeToDraft(sessionId: string, handlers: DraftEventHandlers): () => void;
}

// ── Implementation ────────────────────────────────────────────────────────

function deriveWsUrl(apiUrl: string): string {
  return apiUrl
    .replace(/^https:\/\//, "wss://")
    .replace(/^http:\/\//, "ws://")
    // Strip trailing /api if present so we connect to the root host
    .replace(/\/api\/?$/, "");
}

class WebSocketRealtimeService implements RealtimeService {
  private wsBaseUrl: string;

  constructor() {
    this.wsBaseUrl = deriveWsUrl(API_URL);
  }

  subscribeToDraft(sessionId: string, handlers: DraftEventHandlers): () => void {
    let ws: WebSocket | null = null;
    let reconnectTimeout: ReturnType<typeof setTimeout> | null = null;
    let backoff = 1000;
    let closed = false;

    const connect = () => {
      if (closed) return;

      const token = authService.getToken();
      const url = `${this.wsBaseUrl}/ws/draft/${sessionId}${token ? `?token=${token}` : ""}`;

      try {
        ws = new WebSocket(url);
      } catch (e) {
        console.error("[WS] Failed to construct WebSocket:", e);
        return;
      }

      ws.onopen = () => {
        backoff = 1000;
      };

      ws.onmessage = (event) => {
        try {
          const message = JSON.parse(event.data);
          switch (message.type) {
            case "sessionUpdated":
              handlers.onSessionUpdated?.(message.data);
              break;
            case "pickMade":
              // Backend sends {pick: {...}}, unwrap it
              handlers.onPickMade?.(message.data?.pick ?? message.data);
              break;
            case "sleeperUpdated":
              handlers.onSleeperUpdated?.();
              break;
          }
        } catch {
          // ignore parse errors
        }
      };

      ws.onclose = () => {
        if (closed) return;
        reconnectTimeout = setTimeout(() => {
          backoff = Math.min(backoff * 2, 30000);
          connect();
        }, backoff);
      };

      ws.onerror = (e) => {
        console.error("[WS] Error:", e);
      };
    };

    connect();

    // Return unsubscribe function
    return () => {
      closed = true;
      if (reconnectTimeout) clearTimeout(reconnectTimeout);
      if (ws) {
        ws.onclose = null;
        ws.close();
      }
    };
  }
}

export const realtimeService = new WebSocketRealtimeService();
