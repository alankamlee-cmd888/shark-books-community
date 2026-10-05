const crypto = require('crypto');
const { spawn } = require('child_process');
const pdfMake = require('pdfmake/build/pdfmake');
const vf = require('pdfmake/build/vfs_fonts');

pdfMake.vfs = (vf && vf.pdfMake && vf.pdfMake.vfs) ||
  (vf && vf.default && vf.default.pdfMake && vf.default.pdfMake.vfs) || vf;

const rows = [['Item', 'Description', 'Amount']];
for (let i = 1; i <= 140; i++) {
  rows.push([String(i), 'Café naïve – £ € – Ελληνικά ' + i, '£' + (i * 1.23).toFixed(2)]);
}

const doc = {
  info: {
    title: 'SBC8A4 deterministic fixture',
    author: 'MTD Shark',
    subject: 'renderer bakeoff',
    keywords: 'local embedded',
    creationDate: new Date('2026-10-02T20:00:00Z'),
    modDate: new Date('2026-10-02T20:00:00Z')
  },
  header: (p, pc) => ({ text: 'MTD Shark invoice fixture · ' + p + '/' + pc, alignment: 'right', margin: [0, 12, 24, 0] }),
  footer: (p, pc) => ({ text: 'Page ' + p + ' of ' + pc, alignment: 'center', margin: [0, 0, 0, 12] }),
  content: [
    { text: 'Invoice INV-2026-0001', style: 'title' },
    { text: 'Unicode fixture: Café naïve — £ € — Ελληνικά — Привет' },
    { qr: 'LOCAL-OFFLINE-INV-2026-0001', fit: 80 },
    { text: 'Long table', pageBreak: 'before', style: 'heading' },
    { table: { headerRows: 1, widths: [40, '*', 80], body: rows }, layout: 'lightHorizontalLines' }
  ],
  styles: { title: { fontSize: 20, bold: true }, heading: { fontSize: 14, bold: true } },
  defaultStyle: { fontSize: 9 }
};

async function render() {
  const pdf = pdfMake.createPdf(doc);
  return await pdf.getBuffer();
}

async function main() {
  const bridgeExe = process.argv[2];
  const root = process.argv[3];
  if (!bridgeExe || !root) throw new Error('usage: node emit_pdfmake_to_rust.js <bridge.exe> <storage-root>');
  const a = await render();
  const b = await render();
  const ha = crypto.createHash('sha256').update(a).digest('hex');
  const hb = crypto.createHash('sha256').update(b).digest('hex');
  if (ha !== hb || a.length !== b.length) throw new Error('NONDETERMINISTIC_PDF');

  const child = spawn(bridgeExe, [root, 'invoice-proof.pdf'], { stdio: ['pipe', 'pipe', 'pipe'] });
  const stdout = [];
  const stderr = [];
  child.stdout.on('data', d => stdout.push(d));
  child.stderr.on('data', d => stderr.push(d));
  child.stdin.end(a);
  const code = await new Promise((resolve, reject) => {
    child.on('error', reject);
    child.on('close', resolve);
  });
  if (code !== 0) throw new Error('RUST_BRIDGE_FAILED:' + Buffer.concat(stderr).toString('utf8'));
  const receipt = JSON.parse(Buffer.concat(stdout).toString('utf8').trim());
  if (receipt.sha256 !== ha || receipt.bytes !== a.length || receipt.status !== 'PASS') {
    throw new Error('RUST_RECEIPT_MISMATCH');
  }
  console.log(JSON.stringify({
    schema_version: 1,
    candidate: 'pdfmake',
    pdfmake: require('pdfmake/package.json').version,
    deterministic_pdf: true,
    pdf_bytes: a.length,
    pdf_sha256: ha,
    remote_assets_used: false,
    features: ['embedded_vfs_fonts','unicode','qr','long_table_140_rows','repeated_table_header','explicit_page_break','header','footer','fixed_pdf_metadata'],
    rust_save_store: receipt
  }));
}

main().catch(e => { console.error(e && e.stack || String(e)); process.exit(2); });
