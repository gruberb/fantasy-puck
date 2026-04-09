.PHONY: run db-start db-stop db-reset backend frontend install clean check

# Start local dev: Supabase DB + backend (wait for ready) + frontend
run: db-start
	@echo "Starting backend..."
	@cd backend && cp .env.development .env && cargo run -- serve --port 3000 &
	@echo "Waiting for backend to be ready..."
	@until curl -s http://localhost:3000/api/nhl/playoffs > /dev/null 2>&1; do sleep 1; done
	@echo "Backend ready. Starting frontend..."
	@trap 'kill 0' EXIT; \
		cd frontend && npm run dev & \
		wait

# --- Database (Supabase local) ---

db-start:
	@echo "Starting local Supabase database..."
	@cd backend && supabase start --ignore-health-check

db-stop:
	@cd backend && supabase stop

db-reset:
	@echo "Resetting database (reapplies all migrations)..."
	@cd backend && supabase db reset

db-status:
	@cd backend && supabase status

# --- Individual services (for when DB is already running) ---

backend:
	@cd backend && cp .env.development .env && cargo run -- serve --port 3000

frontend:
	@cd frontend && npm run dev

# --- Setup ---

install:
	@echo "Installing frontend dependencies..."
	@cd frontend && npm install
	@echo "Building backend (first run may take a while)..."
	@cd backend && cargo build
	@echo "Done. Start Docker, then run 'make run'."

# --- Utilities ---

check:
	@cd backend && cargo check
	@cd frontend && npx tsc --noEmit
	@echo "All checks passed."

cache-clear:
	@cd backend && echo "DELETE FROM response_cache;" | supabase db query
	@echo "Cache cleared."

clean:
	@cd backend && cargo clean
	@rm -rf frontend/node_modules frontend/dist

# --- Production deploy (uses Fly.io secrets, not local env) ---

deploy-backend:
	@cd backend && fly deploy

deploy-frontend:
	@cd frontend && fly deploy

deploy: deploy-backend deploy-frontend
