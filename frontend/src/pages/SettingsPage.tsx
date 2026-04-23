import { useState, useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { useAuth } from "@/contexts/AuthContext";
import { api } from "@/api/client";
import { authService } from "@/features/auth";
import { PageHeader } from "@gruberb/fun-ui";

const SettingsPage = () => {
  const { user, profile, signOut } = useAuth();
  const navigate = useNavigate();

  const [displayName, setDisplayName] = useState(profile?.displayName ?? "");
  const [saving, setSaving] = useState(false);
  const [flash, setFlash] = useState<string | null>(null);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  const [deleteConfirmText, setDeleteConfirmText] = useState("");
  const [deleting, setDeleting] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);

  useEffect(() => {
    setDisplayName(profile?.displayName ?? "");
  }, [profile?.displayName]);

  const showFlash = (msg: string) => {
    setFlash(msg);
    setTimeout(() => setFlash(null), 3000);
  };

  const handleSaveDisplayName = async () => {
    if (!user || !displayName.trim()) return;
    setSaving(true);
    try {
      await api.updateProfile(displayName.trim());
      // Update the stored session so the nav/profile reflects the change immediately
      authService.updateSessionProfile({ displayName: displayName.trim(), isAdmin: profile?.isAdmin ?? false });
      showFlash("Display name updated!");
    } catch {
      // ignore
    }
    setSaving(false);
  };

  const handleDeleteAccount = async () => {
    if (deleteConfirmText !== "DELETE") return;
    setDeleting(true);
    setDeleteError(null);
    try {
      await api.deleteAccount();
      await signOut();
      navigate("/login");
    } catch {
      setDeleteError("Failed to delete account. Please try again.");
      setDeleting(false);
    }
  };

  if (!user) return null;

  return (
    <div className="max-w-2xl mx-auto space-y-8">
      <PageHeader title="Settings" subtitle={user.email} />

      {flash && (
        <div className="fixed bottom-6 left-1/2 -translate-x-1/2 z-50 bg-[#1A1A1A] text-white px-6 py-3 border-2 border-[#16A34A] text-sm font-bold uppercase tracking-wider shadow-[4px_4px_0px_0px_#16A34A]">
          {flash}
        </div>
      )}

      {/* Profile */}
      <div className="border-2 border-[#1A1A1A] bg-white">
        <div className="px-6 py-4 border-b-2 border-[#1A1A1A]">
          <h2 className="text-lg font-bold">Profile</h2>
        </div>
        <div className="p-6">
          <label className="block text-xs font-bold uppercase tracking-wider text-gray-500 mb-2">
            Display Name
          </label>
          <div className="flex gap-3">
            <input
              type="text"
              value={displayName}
              onChange={(e) => setDisplayName(e.target.value)}
              className="flex-1 px-4 py-2 border-2 border-[#1A1A1A] rounded-none focus:outline-none focus:ring-2 focus:ring-[#2563EB] font-medium"
            />
            <button
              onClick={handleSaveDisplayName}
              disabled={saving || displayName === profile?.displayName}
              className="btn-gradient disabled:opacity-40"
            >
              {saving ? "Saving..." : "Save"}
            </button>
          </div>
        </div>
      </div>

      {/* Danger Zone */}
      <div className="border-2 border-red-500 bg-white">
        <div className="px-6 py-4 border-b-2 border-red-500">
          <h2 className="text-lg font-bold text-red-600">Danger Zone</h2>
        </div>
        <div className="p-6">
          {!showDeleteConfirm ? (
            <div className="flex items-center justify-between">
              <div>
                <p className="font-bold text-sm">Delete Account</p>
                <p className="text-xs text-gray-500 mt-1">
                  Permanently delete your account and all associated data. This cannot be undone.
                </p>
              </div>
              <button
                onClick={() => setShowDeleteConfirm(true)}
                className="px-4 py-2 bg-white text-red-600 border-2 border-red-500 text-xs font-bold uppercase tracking-wider hover:bg-red-50 transition-colors"
              >
                Delete Account
              </button>
            </div>
          ) : (
            <div className="space-y-4">
              <p className="text-sm text-gray-700">
                This will permanently delete your account, all your fantasy teams, players, and league memberships.
                Type <span className="font-bold">DELETE</span> to confirm.
              </p>
              <div className="flex gap-3">
                <input
                  type="text"
                  value={deleteConfirmText}
                  onChange={(e) => setDeleteConfirmText(e.target.value)}
                  placeholder="Type DELETE to confirm"
                  className="flex-1 px-4 py-2 border-2 border-red-300 rounded-none focus:outline-none focus:ring-2 focus:ring-red-500 font-medium text-sm"
                  autoFocus
                />
                <button
                  onClick={handleDeleteAccount}
                  disabled={deleteConfirmText !== "DELETE" || deleting}
                  className="px-4 py-2 bg-red-600 text-white border-2 border-red-700 text-xs font-bold uppercase tracking-wider disabled:opacity-40 hover:bg-red-700 transition-colors"
                >
                  {deleting ? "Deleting..." : "Confirm Delete"}
                </button>
                <button
                  onClick={() => {
                    setShowDeleteConfirm(false);
                    setDeleteConfirmText("");
                    setDeleteError(null);
                  }}
                  className="px-4 py-2 bg-white text-[#1A1A1A] border-2 border-[#1A1A1A] text-xs font-bold uppercase tracking-wider"
                >
                  Cancel
                </button>
              </div>
              {deleteError && (
                <p className="text-sm text-red-600 font-medium">{deleteError}</p>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
};

export default SettingsPage;
