export interface AuthUser {
  id: string;
  email: string;
}

export interface AuthProfile {
  displayName: string;
  isAdmin: boolean;
}

export interface AuthSession {
  user: AuthUser;
  profile: AuthProfile;
  token: string;
}

export interface AuthService {
  login(email: string, password: string): Promise<AuthSession>;
  register(email: string, password: string, displayName: string): Promise<AuthSession>;
  logout(): Promise<void>;
  getSession(): AuthSession | null;
  getToken(): string | null;
  onAuthStateChange(callback: (session: AuthSession | null) => void): () => void;
}
