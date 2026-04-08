import { API_URL } from '@/config';
import { authService } from '@/features/auth';

async function fetchApi<T>(
  endpoint: string,
  options?: { method?: string; body?: unknown; fallback?: T },
): Promise<T> {
  const url = `${API_URL}${endpoint.startsWith("/") ? endpoint : `/${endpoint}`}`;
  const token = authService.getToken();

  const headers: Record<string, string> = { "Content-Type": "application/json" };
  if (token) headers["Authorization"] = `Bearer ${token}`;

  const method = options?.method ?? "GET";
  const fallback = options?.fallback;

  try {
    const response = await fetch(url, {
      method,
      headers,
      body: options?.body ? JSON.stringify(options.body) : undefined,
    });

    // If the server is down or rate limited, return fallback if available
    if (!response.ok && fallback !== undefined) {
      console.warn(`[API] ${endpoint} → ${response.status}, using fallback`);
      return fallback;
    }

    // For DELETE requests that return 204 No Content
    if (response.status === 204) {
      return undefined as T;
    }

    const jsonData = await response.json();

    if (!jsonData.success) {
      // If we have a fallback, use it instead of throwing
      if (fallback !== undefined) {
        console.warn(
          `[API] ${endpoint} → error: ${jsonData.error}, using fallback`,
        );
        return fallback;
      }
      throw new Error(jsonData.error || "API request failed");
    }

    return jsonData.data as T;
  } catch (error) {
    if (fallback !== undefined) {
      console.warn(`[API] ${endpoint} → fetch failed, using fallback`);
      return fallback;
    }
    console.error(`[API] ${endpoint} → FAILED:`, error);
    throw error;
  }
}

// Helper to append league_id query param
function withLeague(endpoint: string, leagueId: string): string {
  const separator = endpoint.includes("?") ? "&" : "?";
  return `${endpoint}${separator}league_id=${leagueId}`;
}

export { fetchApi, withLeague };
