.PHONY: run dev db-start db-stop db-reset backend frontend install clean

# Start everything: database + backend + frontend
run: db-start
	@echo "Starting backend and frontend..."
	@trap 'kill 0' EXIT; \
		$(MAKE) backend & \
		$(MAKE) frontend & \
		wait

# Alias
dev: run

# --- Database (Supabase local) ---

db-start:
	@echo "Starting local Supabase database..."
	@cd backend && supabase start --ignore-health-check 2>/dev/null || true
	@echo "Database ready at postgresql://postgres:postgres@127.0.0.1:54322/postgres"

db-stop:
	@cd backend && supabase stop

db-reset:
	@echo "Resetting database (reapplies all migrations)..."
	@cd backend && supabase db reset

db-status:
	@cd backend && supabase status

# --- Backend (Rust/Axum) ---

backend:
	@echo "Starting backend on :3000..."
	@cd backend && cargo run -- serve --port 3000

backend-check:
	@cd backend && cargo check

# --- Frontend (React/Vite) ---

frontend:
	@echo "Starting frontend on :5173..."
	@cd frontend && npm run dev

frontend-check:
	@cd frontend && npx tsc --noEmit

# --- Setup ---

install:
	@echo "Installing dependencies..."
	@cd frontend && npm install
	@echo "Building backend (first run may take a while)..."
	@cd backend && cargo build
	@echo "Done. Run 'make run' to start."

# --- Utilities ---

check: backend-check frontend-check
	@echo "All checks passed."

clean:
	@cd backend && cargo clean
	@rm -rf frontend/node_modules frontend/dist
