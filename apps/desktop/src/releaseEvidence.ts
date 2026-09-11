import type { AcceptanceGate, AcceptanceSnapshot } from "./acceptanceChecklist";
import type { ReadinessLevel, RuntimeReadinessReport } from "./types";

export interface ReleaseEvidenceCheck {
  id: string;
  label: string;
  level: ReadinessLevel;
}

export interface ReleaseEvidence {
  schemaVersion: 1;
  generatedAtUnix: number;
  generatedAtIso: string;
  gate: AcceptanceGate;
  checklist: {
    total: number;
    requiredTotal: number;
    requiredPassed: number;
    passed: number;
    failed: number;
    blocked: number;
    pending: number;
    items: Array<{
      id: string;
      number: number;
      category: string;
      label: string;
      required: boolean;
      status: string;
      updatedUnix: number;
    }>;
  };
  runtime: {
    overall: ReadinessLevel;
    ready: number;
    optionalMissing: number;
    blocking: number;
    checks: ReleaseEvidenceCheck[];
    satellite: {
      enabled: boolean;
      paired: boolean;
      credentialStorage: string;
      environmentTokenOverride: boolean;
      remoteManaged: boolean;
      remotePort: number | null;
      trustedDevices: number;
      revokedDevices: number;
      warningCount: number;
    };
  };
  redactions: readonly string[];
}

function countLevel(readiness: RuntimeReadinessReport, level: ReadinessLevel): number {
  return readiness.checks.filter((check) => check.level === level).length;
}

export function buildReleaseEvidence(
  checklist: AcceptanceSnapshot,
  readiness: RuntimeReadinessReport,
  gate: AcceptanceGate,
  generatedAtUnix = Math.floor(Date.now() / 1000),
): ReleaseEvidence {
  if (!Number.isFinite(generatedAtUnix) || generatedAtUnix <= 0) {
    throw new Error("Release evidence timestamp không hợp lệ.");
  }

  return {
    schemaVersion: 1,
    generatedAtUnix,
    generatedAtIso: new Date(generatedAtUnix * 1000).toISOString(),
    gate,
    checklist: {
      total: checklist.total,
      requiredTotal: checklist.requiredTotal,
      requiredPassed: checklist.requiredPassed,
      passed: checklist.passed,
      failed: checklist.failed,
      blocked: checklist.blocked,
      pending: checklist.pending,
      items: checklist.items.map((item) => ({
        id: item.id,
        number: item.number,
        category: item.category,
        label: item.label,
        required: item.required,
        status: item.status,
        updatedUnix: item.updatedUnix,
      })),
    },
    runtime: {
      overall: readiness.overall,
      ready: countLevel(readiness, "ready"),
      optionalMissing: countLevel(readiness, "optional_missing"),
      blocking: countLevel(readiness, "blocking"),
      checks: readiness.checks.map((check) => ({
        id: check.id,
        label: check.label,
        level: check.level,
      })),
      satellite: {
        enabled: readiness.satellite.enabled,
        paired: readiness.satellite.paired,
        credentialStorage: readiness.satellite.credential_storage,
        environmentTokenOverride: readiness.satellite.environment_token_override,
        remoteManaged: readiness.satellite.remote_managed,
        remotePort: readiness.satellite.remote_port ?? null,
        trustedDevices: readiness.satellite.trusted_devices,
        revokedDevices: readiness.satellite.revoked_devices,
        warningCount: readiness.satellite.warnings.length,
      },
    },
    redactions: [
      "pairing credentials",
      "satellite device identifiers and names",
      "runtime filesystem paths",
      "runtime check detail strings",
    ],
  };
}

export function serializeReleaseEvidence(evidence: ReleaseEvidence): string {
  return `${JSON.stringify(evidence, null, 2)}\n`;
}

export function releaseEvidenceFilename(generatedAtUnix: number): string {
  const stamp = new Date(generatedAtUnix * 1000)
    .toISOString()
    .replace(/[-:]/g, "")
    .replace(/\.\d{3}Z$/, "Z");
  return `assistant-release-evidence-${stamp}.json`;
}
