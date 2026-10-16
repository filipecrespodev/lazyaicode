export type ConflictedFile = {
  path: string;
  ancestorContent: string | null;
  oursContent: string | null;
  theirsContent: string | null;
  isBinary: boolean;
  patch: string;
};

export type RebaseStatus = {
  state: "Conflicts" | "Complete";
  currentStep: number;
  totalSteps: number;
  conflicts: ConflictedFile[];
};

export type ConflictResolution =
  | { kind: "UseOurs" }
  | { kind: "UseTheirs" }
  | { kind: "UseManualContent"; content: string };
