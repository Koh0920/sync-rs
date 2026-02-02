/**
 * ManifestPanel - Display manifest metadata
 */

import React from 'react';
import { 
  FileText, 
  Clock, 
  User, 
  Shield, 
  Network, 
  CheckCircle2, 
  AlertTriangle,
  Hash
} from 'lucide-react';
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/Card';
import type { SyncManifest } from '@/types/sync';
import { formatBytes, formatDate } from '@/lib/content-renderer';
import { isExpired, formatTtl, getExpiresIn } from '@/lib/sync-parser';

interface ManifestPanelProps {
  manifest: SyncManifest;
  fileName: string;
  fileSize: number;
}

export function ManifestPanel({ manifest, fileName, fileSize }: ManifestPanelProps) {
  const expired = isExpired(manifest);
  const expiresIn = getExpiresIn(manifest);

  return (
    <Card>
      <CardHeader className="pb-3">
        <CardTitle className="flex items-center gap-2 text-base">
          <FileText className="h-4 w-4" />
          Manifest
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        {/* File Info */}
        <Section title="File Info">
          <InfoRow label="Name" value={fileName} />
          <InfoRow label="Size" value={formatBytes(fileSize)} />
          <InfoRow label="Format" value={`v${manifest.sync.version}`} />
          <InfoRow label="Content Type" value={manifest.sync.content_type} />
          <InfoRow label="Extension" value={`.${manifest.sync.display_ext}`} />
        </Section>

        {/* Metadata */}
        <Section title="Metadata" icon={<User className="h-3.5 w-3.5" />}>
          <InfoRow label="Created By" value={manifest.meta.created_by} />
          <InfoRow label="Created At" value={formatDate(manifest.meta.created_at)} />
          <InfoRow 
            label="Hash Algorithm" 
            value={manifest.meta.hash_algo}
            icon={<Hash className="h-3 w-3" />}
          />
        </Section>

        {/* Policy */}
        <Section title="Policy" icon={<Clock className="h-3.5 w-3.5" />}>
          <InfoRow 
            label="TTL" 
            value={formatTtl(manifest.policy.ttl)}
          />
          <InfoRow 
            label="Status"
            value={expired ? 'Expired' : `Expires in ${formatTtl(Math.floor(expiresIn / 1000))}`}
            valueClassName={expired ? 'text-red-600' : 'text-green-600'}
            icon={expired ? <AlertTriangle className="h-3 w-3" /> : <CheckCircle2 className="h-3 w-3" />}
          />
          <InfoRow label="Timeout" value={`${manifest.policy.timeout}s`} />
        </Section>

        {/* Permissions */}
        <Section title="Permissions" icon={<Shield className="h-3.5 w-3.5" />}>
          {manifest.permissions.allow_hosts.length > 0 ? (
            <InfoRow 
              label="Allowed Hosts" 
              value={manifest.permissions.allow_hosts.join(', ')}
            />
          ) : (
            <InfoRow label="Network" value="No network access" valueClassName="text-neutral-400" />
          )}
          {manifest.permissions.allow_env.length > 0 && (
            <InfoRow 
              label="Environment" 
              value={manifest.permissions.allow_env.join(', ')}
            />
          )}
        </Section>

        {/* Ownership */}
        <Section title="Ownership" icon={<Network className="h-3.5 w-3.5" />}>
          <InfoRow 
            label="Owner Capsule" 
            value={manifest.ownership.owner_capsule || 'None'}
            valueClassName={!manifest.ownership.owner_capsule ? 'text-neutral-400' : undefined}
          />
          <InfoRow 
            label="Write Allowed" 
            value={manifest.ownership.write_allowed ? 'Yes' : 'No'}
            valueClassName={manifest.ownership.write_allowed ? 'text-green-600' : 'text-neutral-400'}
          />
        </Section>

        {/* Verification */}
        {manifest.verification.enabled && (
          <Section title="Verification" icon={<CheckCircle2 className="h-3.5 w-3.5" />}>
            <InfoRow label="VM Type" value={manifest.verification.vm_type || 'N/A'} />
            <InfoRow label="Proof Type" value={manifest.verification.proof_type || 'N/A'} />
          </Section>
        )}
      </CardContent>
    </Card>
  );
}

function Section({ 
  title, 
  icon, 
  children 
}: { 
  title: string; 
  icon?: React.ReactNode; 
  children: React.ReactNode;
}) {
  return (
    <div className="space-y-2">
      <h4 className="flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wider text-neutral-500">
        {icon}
        {title}
      </h4>
      <div className="space-y-1">
        {children}
      </div>
    </div>
  );
}

function InfoRow({ 
  label, 
  value, 
  valueClassName,
  icon 
}: { 
  label: string; 
  value: string;
  valueClassName?: string;
  icon?: React.ReactNode;
}) {
  return (
    <div className="flex items-center justify-between text-sm">
      <span className="text-neutral-500">{label}</span>
      <span className={`font-mono text-xs flex items-center gap-1 ${valueClassName || 'text-neutral-900'}`}>
        {icon}
        {value}
      </span>
    </div>
  );
}
