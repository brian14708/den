"use client";

import { useCallback, useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { registerPasskey } from "@/lib/webauthn";

interface Passkey {
  id: number;
  name: string;
  created: string;
  last_used: string | null;
}

interface PasskeyListProps {
  onUnauthorized?: () => void;
}

function formatDate(iso: string): string {
  return new Date(iso + "Z").toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}

export function PasskeyList({ onUnauthorized }: PasskeyListProps) {
  const [passkeys, setPasskeys] = useState<Passkey[]>([]);
  const [loading, setLoading] = useState(true);
  const [editingId, setEditingId] = useState<number | null>(null);
  const [editName, setEditName] = useState("");
  const [deleteTarget, setDeleteTarget] = useState<Passkey | null>(null);
  const [adding, setAdding] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchPasskeys = useCallback(async () => {
    try {
      const res = await fetch("/api/auth/passkeys");
      if (res.status === 401) {
        onUnauthorized?.();
        return;
      }
      if (!res.ok) throw new Error("Failed to load passkeys");
      setPasskeys(await res.json());
    } catch {
      setError("Failed to load passkeys");
    } finally {
      setLoading(false);
    }
  }, [onUnauthorized]);

  useEffect(() => {
    fetchPasskeys();
  }, [fetchPasskeys]);

  const handleRename = async (id: number) => {
    const trimmed = editName.trim();
    if (!trimmed) return;
    try {
      const res = await fetch(`/api/auth/passkeys/${id}/name`, {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ name: trimmed }),
      });
      if (res.status === 401) {
        onUnauthorized?.();
        return;
      }
      if (!res.ok) throw new Error("Rename failed");
      setEditingId(null);
      await fetchPasskeys();
    } catch {
      setError("Failed to rename passkey");
    }
  };

  const handleDelete = async () => {
    if (!deleteTarget) return;
    try {
      const res = await fetch(`/api/auth/passkeys/${deleteTarget.id}`, {
        method: "DELETE",
      });
      if (res.status === 401) {
        onUnauthorized?.();
        return;
      }
      if (!res.ok) throw new Error("Delete failed");
      setDeleteTarget(null);
      await fetchPasskeys();
    } catch {
      setError("Failed to delete passkey");
    }
  };

  const handleAdd = async () => {
    const name = prompt("Name for this passkey:");
    if (!name?.trim()) return;
    setAdding(true);
    setError(null);
    try {
      await registerPasskey(null, name.trim());
      await fetchPasskeys();
    } catch (error) {
      if (error instanceof Error && error.message === "Unauthorized") {
        onUnauthorized?.();
        return;
      }
      setError("Failed to add passkey");
    } finally {
      setAdding(false);
    }
  };

  if (loading) {
    return <p className="text-muted-foreground text-sm">Loading passkeys...</p>;
  }

  return (
    <div className="space-y-4">
      {error && <p className="text-destructive text-sm">{error}</p>}

      <div className="divide-y rounded-lg border">
        {passkeys.map((pk) => (
          <div
            key={pk.id}
            className="flex items-center justify-between gap-4 px-4 py-3"
          >
            <div className="min-w-0 flex-1">
              {editingId === pk.id ? (
                <form
                  className="flex items-center gap-2"
                  onSubmit={(e) => {
                    e.preventDefault();
                    handleRename(pk.id);
                  }}
                >
                  <Input
                    value={editName}
                    onChange={(e) => setEditName(e.target.value)}
                    className="h-7 text-sm"
                    autoFocus
                  />
                  <Button type="submit" variant="outline" size="sm">
                    Save
                  </Button>
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    onClick={() => setEditingId(null)}
                  >
                    Cancel
                  </Button>
                </form>
              ) : (
                <>
                  <button
                    className="text-sm font-medium hover:underline"
                    onClick={() => {
                      setEditingId(pk.id);
                      setEditName(pk.name);
                    }}
                  >
                    {pk.name}
                  </button>
                  <p className="text-muted-foreground text-xs">
                    Added {formatDate(pk.created)}
                    {pk.last_used && (
                      <> &middot; Last used {formatDate(pk.last_used)}</>
                    )}
                  </p>
                </>
              )}
            </div>
            {editingId !== pk.id && (
              <Button
                variant="ghost"
                size="sm"
                className="text-destructive hover:text-destructive"
                disabled={passkeys.length <= 1}
                onClick={() => setDeleteTarget(pk)}
              >
                Delete
              </Button>
            )}
          </div>
        ))}
      </div>

      <Button variant="outline" onClick={handleAdd} disabled={adding}>
        {adding ? "Adding..." : "Add passkey"}
      </Button>

      <Dialog
        open={deleteTarget !== null}
        onOpenChange={(open) => !open && setDeleteTarget(null)}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Delete passkey</DialogTitle>
            <DialogDescription>
              Are you sure you want to delete &ldquo;{deleteTarget?.name}
              &rdquo;? This cannot be undone.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setDeleteTarget(null)}>
              Cancel
            </Button>
            <Button variant="destructive" onClick={handleDelete}>
              Delete
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
