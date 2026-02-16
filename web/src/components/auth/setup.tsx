"use client";

import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { registerPasskey } from "@/lib/webauthn";

interface SetupProps {
  onComplete: (userName: string) => void;
}

export function Setup({ onComplete }: SetupProps) {
  const [name, setName] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const handleSetup = async () => {
    const trimmedName = name.trim();
    if (!trimmedName) return;
    setLoading(true);
    setError(null);
    try {
      await registerPasskey(trimmedName, "initial");
      onComplete(trimmedName);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Setup failed");
    } finally {
      setLoading(false);
    }
  };

  return (
    <Card className="w-full max-w-sm">
      <CardHeader>
        <CardTitle>Welcome to den</CardTitle>
        <CardDescription>Set up your account with a passkey</CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="space-y-2">
          <Label htmlFor="name">Your name</Label>
          <Input
            id="name"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="Enter your name"
            onKeyDown={(e) => e.key === "Enter" && handleSetup()}
          />
        </div>
        {error && <p className="text-destructive text-sm">{error}</p>}
        <Button
          onClick={handleSetup}
          disabled={loading || !name.trim()}
          className="w-full"
        >
          {loading ? "Setting up..." : "Register passkey"}
        </Button>
      </CardContent>
    </Card>
  );
}
