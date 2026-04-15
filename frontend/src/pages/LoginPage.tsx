import { useState } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import { useAuth } from "@/contexts/AuthContext";

type Tab = "login" | "signup";

const LoginPage = () => {
  const [activeTab, setActiveTab] = useState<Tab>("login");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const { signIn, signUp } = useAuth();
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setSubmitting(true);

    try {
      if (activeTab === "login") {
        await signIn(email, password);
      } else {
        await signUp(email, password, displayName);
      }
      const returnTo = searchParams.get("returnTo");
      navigate(returnTo && returnTo.startsWith("/") ? returnTo : "/");
    } catch (err: unknown) {
      if (err instanceof Error) {
        setError(err.message);
      } else {
        setError("An unexpected error occurred.");
      }
    } finally {
      setSubmitting(false);
    }
  };

  const switchTab = (tab: Tab) => {
    setActiveTab(tab);
    setError(null);
  };

  return (
    <div className="min-h-screen flex items-center justify-center bg-[#1A1A1A] px-4 relative">
      {/* Big X in top-right corner of screen */}
      <button
        type="button"
        onClick={() => navigate("/")}
        className="absolute top-6 right-8 text-white text-4xl font-bold hover:text-[#FACC15] transition-colors"
      >
        &times;
      </button>

      <div className="w-full max-w-md">
        <div className="text-center mb-8">
          <h1 className="text-3xl font-extrabold uppercase tracking-wider">
            <span className="text-white">FANTASY</span>{" "}
            <span className="text-[#FACC15]">NHL 2026</span>
          </h1>
        </div>

        <div className="bg-white border-3 border-[#1A1A1A] rounded-none shadow-[8px_8px_0px_0px_#FACC15] overflow-hidden">
          {/* Tabs */}
          <div className="flex">
            <button
              type="button"
              onClick={() => switchTab("login")}
              className={`flex-1 py-3 text-sm font-bold uppercase tracking-wider cursor-pointer transition-colors duration-100 rounded-none ${
                activeTab === "login"
                  ? "bg-[#2563EB] text-white"
                  : "bg-[#F5F0E8] text-[#1A1A1A] hover:bg-gray-200"
              }`}
            >
              Login
            </button>
            <button
              type="button"
              onClick={() => switchTab("signup")}
              className={`flex-1 py-3 text-sm font-bold uppercase tracking-wider cursor-pointer transition-colors duration-100 rounded-none ${
                activeTab === "signup"
                  ? "bg-[#2563EB] text-white"
                  : "bg-[#F5F0E8] text-[#1A1A1A] hover:bg-gray-200"
              }`}
            >
              Sign Up
            </button>
          </div>

          {/* Form */}
          <form onSubmit={handleSubmit} className="p-6 space-y-4">
            {activeTab === "signup" && (
              <div>
                <label
                  htmlFor="displayName"
                  className="block text-sm font-bold text-[#1A1A1A] uppercase tracking-wider mb-1"
                >
                  Display Name
                </label>
                <input
                  id="displayName"
                  type="text"
                  value={displayName}
                  onChange={(e) => setDisplayName(e.target.value)}
                  required
                  className="w-full px-4 py-2 border-2 border-[#1A1A1A] rounded-none focus:outline-none focus:ring-2 focus:ring-[#2563EB] focus:border-[#2563EB] transition-all duration-100"
                  placeholder="Your display name"
                />
              </div>
            )}

            <div>
              <label
                htmlFor="email"
                className="block text-sm font-bold text-[#1A1A1A] uppercase tracking-wider mb-1"
              >
                Email
              </label>
              <input
                id="email"
                type="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                required
                className="w-full px-4 py-2 border-2 border-[#1A1A1A] rounded-none focus:outline-none focus:ring-2 focus:ring-[#2563EB] focus:border-[#2563EB] transition-all duration-100"
                placeholder="you@example.com"
              />
            </div>

            <div>
              <label
                htmlFor="password"
                className="block text-sm font-bold text-[#1A1A1A] uppercase tracking-wider mb-1"
              >
                Password
              </label>
              <input
                id="password"
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                required
                className="w-full px-4 py-2 border-2 border-[#1A1A1A] rounded-none focus:outline-none focus:ring-2 focus:ring-[#2563EB] focus:border-[#2563EB] transition-all duration-100"
                placeholder="Enter your password"
              />
            </div>

            {error && (
              <div className="bg-[#EF4444]/10 border-2 border-[#EF4444] rounded-none text-red-700 px-4 py-3 text-sm">
                {error}
              </div>
            )}

            <button
              type="submit"
              disabled={submitting}
              className="w-full py-3 bg-[#2563EB] text-white border-2 border-[#1A1A1A] font-bold uppercase tracking-wider rounded-none shadow-[4px_4px_0px_0px_#1A1A1A] hover:translate-x-[2px] hover:translate-y-[2px] hover:shadow-none disabled:opacity-50 disabled:cursor-not-allowed transition-all duration-100"
            >
              {submitting
                ? "Please wait..."
                : activeTab === "login"
                  ? "Sign In"
                  : "Create Account"}
            </button>
          </form>
        </div>
      </div>
    </div>
  );
};

export default LoginPage;
