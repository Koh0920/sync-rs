/**
 * PayloadViewer - Display payload content based on content type
 */

import { useMemo } from 'react';
import { FileText, Image, Code, Table, Binary, Download } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { renderPayload, formatBytes } from '@/lib/content-renderer';
import type { ContentCategory } from '@/types/sync';

interface PayloadViewerProps {
  payload: Uint8Array;
  contentType: string;
  displayExt: string;
  fileName: string;
}

export function PayloadViewer({ payload, contentType, displayExt, fileName }: PayloadViewerProps) {
  const content = useMemo(
    () => renderPayload(payload, contentType),
    [payload, contentType]
  );

  const handleDownload = () => {
    // Create a copy of the buffer to ensure it's a plain ArrayBuffer
    const buffer = payload.slice().buffer;
    const blob = new Blob([buffer], { type: contentType });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = fileName.replace('.sync', `.${displayExt}`);
    a.click();
    URL.revokeObjectURL(url);
  };

  return (
    <Card className="flex flex-col h-full">
      <CardHeader className="pb-3 flex-shrink-0">
        <div className="flex items-center justify-between">
          <CardTitle className="flex items-center gap-2 text-base">
            <CategoryIcon category={content.category} />
            Payload
          </CardTitle>
          <div className="flex items-center gap-2">
            <span className="text-xs text-neutral-500">
              {formatBytes(payload.length)} • {contentType}
            </span>
            <Button variant="outline" size="sm" onClick={handleDownload}>
              <Download className="h-4 w-4 mr-1" />
              Download
            </Button>
          </div>
        </div>
      </CardHeader>
      <CardContent className="flex-1 overflow-hidden">
        <PayloadContent content={content} />
      </CardContent>
    </Card>
  );
}

function CategoryIcon({ category }: { category: ContentCategory }) {
  switch (category) {
    case 'text':
      return <FileText className="h-4 w-4" />;
    case 'image':
      return <Image className="h-4 w-4" />;
    case 'json':
      return <Code className="h-4 w-4" />;
    case 'csv':
      return <Table className="h-4 w-4" />;
    default:
      return <Binary className="h-4 w-4" />;
  }
}

function PayloadContent({ content }: { content: ReturnType<typeof renderPayload> }) {
  switch (content.category) {
    case 'text':
      return (
        <pre className="h-full overflow-auto rounded-lg bg-neutral-50 p-4 text-sm font-mono whitespace-pre-wrap">
          {content.text}
        </pre>
      );

    case 'json':
      return (
        <pre className="h-full overflow-auto rounded-lg bg-neutral-900 p-4 text-sm font-mono text-green-400">
          {JSON.stringify(content.json, null, 2)}
        </pre>
      );

    case 'csv':
      return content.csv ? (
        <div className="h-full overflow-auto">
          <table className="min-w-full border-collapse text-sm">
            <thead>
              <tr className="bg-neutral-100">
                {content.csv[0]?.map((cell, i) => (
                  <th key={i} className="border border-neutral-200 px-3 py-2 text-left font-semibold">
                    {cell}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {content.csv.slice(1).map((row, i) => (
                <tr key={i} className={i % 2 === 0 ? 'bg-white' : 'bg-neutral-50'}>
                  {row.map((cell, j) => (
                    <td key={j} className="border border-neutral-200 px-3 py-2">
                      {cell}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <pre className="h-full overflow-auto rounded-lg bg-neutral-50 p-4 text-sm font-mono">
          {content.text}
        </pre>
      );

    case 'image':
      return content.dataUrl ? (
        <div className="flex h-full items-center justify-center bg-neutral-100 rounded-lg p-4">
          <img 
            src={content.dataUrl} 
            alt="Payload" 
            className="max-w-full max-h-full object-contain"
          />
        </div>
      ) : (
        <div className="flex h-full items-center justify-center text-neutral-400">
          Unable to render image
        </div>
      );

    case 'binary':
    default:
      return (
        <div className="h-full overflow-auto">
          <HexDump bytes={content.bytes || new Uint8Array()} />
        </div>
      );
  }
}

function HexDump({ bytes }: { bytes: Uint8Array }) {
  const rows = useMemo(() => {
    const result: Array<{ offset: string; hex: string; ascii: string }> = [];
    const bytesPerRow = 16;
    const maxRows = 100; // Limit to prevent performance issues

    for (let i = 0; i < Math.min(bytes.length, maxRows * bytesPerRow); i += bytesPerRow) {
      const slice = bytes.slice(i, i + bytesPerRow);
      const offset = i.toString(16).padStart(8, '0');
      const hex = Array.from(slice)
        .map(b => b.toString(16).padStart(2, '0'))
        .join(' ');
      const ascii = Array.from(slice)
        .map(b => (b >= 32 && b <= 126 ? String.fromCharCode(b) : '.'))
        .join('');

      result.push({ offset, hex, ascii });
    }

    return result;
  }, [bytes]);

  return (
    <div className="rounded-lg bg-neutral-900 p-4 font-mono text-xs">
      {rows.map((row, i) => (
        <div key={i} className="flex gap-4">
          <span className="text-blue-400">{row.offset}</span>
          <span className="text-neutral-300 flex-1">{row.hex}</span>
          <span className="text-green-400">{row.ascii}</span>
        </div>
      ))}
      {bytes.length > rows.length * 16 && (
        <div className="mt-2 text-neutral-500">
          ... {bytes.length - rows.length * 16} more bytes
        </div>
      )}
    </div>
  );
}
