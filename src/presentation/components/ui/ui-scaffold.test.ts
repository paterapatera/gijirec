import { describe, expect, test } from "bun:test";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Separator } from "@/components/ui/separator";
import { Toaster } from "@/components/ui/sonner";
import { Switch } from "@/components/ui/switch";
import { cn } from "@/lib/utils";

describe("ui scaffold", () => {
  test("cn() merges class names", () => {
    expect(cn("px-2", "px-4")).toBe("px-4");
    expect(cn("text-sm", undefined, "font-medium")).toBe("text-sm font-medium");
  });

  test("shadcn ui components are importable", () => {
    expect(Button).toBeDefined();
    expect(Switch).toBeDefined();
    expect(Label).toBeDefined();
    expect(Separator).toBeDefined();
    expect(Alert).toBeDefined();
    expect(AlertTitle).toBeDefined();
    expect(AlertDescription).toBeDefined();
    expect(Toaster).toBeDefined();
  });
});
