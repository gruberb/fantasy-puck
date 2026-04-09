.PHONY: run dev db-start db-stop db-reset backend frontend install clean check

# Start backend + frontend (no local DB — uses whatever DATABASE_URL is configured)
run:
	@echo "Starting backend and frontend..."
	@trap 'kill 0' EXIT; \
		cd backend && cargo run -- serve --port 3000 & \
		cd frontend && npm run dev & \
		wait

# Start everything: local database + backend + frontend
dev:
	@$(MAKE) db-start
	@$(MAKE) run

# --- Database (Supabase local) ---

db-start:
	@echo "Starting local Supabase database..."
	@cd backend && supabase start --ignore-health-check
	@echo "Database ready at postgresql://postgres:postgres@127.0.0.1:54322/postgres"

db-stop:
	@cd backend && supabase stop

db-reset:
	@echo "Resetting database (reapplies all migrations)..."
	@cd backend && supabase db reset

db-status:
	@cd backend && supabase status

# --- Individual services ---

backend:
	@cd backend && cargo run -- serve --port 3000

frontend:
	@cd frontend && npm run dev

# --- Setup ---

install:
	@echo "Installing frontend dependencies..."
	@cd frontend && npm install
	@echo "Building backend (first run may take a while)..."
	@cd backend && cargo build
	@echo "Done. Run 'make run' to start."

# --- Utilities ---

check:
	@cd backend && cargo check
	@cd frontend && npx tsc --noEmit
	@echo "All checks passed."

clean:
	@cd backend && cargo clean
	@rm -rf frontend/node_modules frontend/dist
