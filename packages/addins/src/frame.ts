/**
 * How an add-in page is framed (CORE-ADDIN-001, ADR-0014): a sandboxed iframe
 * (`allow-scripts allow-forms`; `allow-same-origin` only for add-ins an
 * administrator marked trusted; never top navigation, popups or downloads), a
 * required Content Security Policy (`csp` attribute, CSP Embedded Enforcement)
 * that limits where the page loads from and what it may contact (`network:`
 * permissions), no powerful features (camera, microphone…) and no referrer
 * beyond the origin.
 */
import { networkOrigins, type AddinManifest } from '@blueoffice/addin-sdk';

export interface FrameAttributes {
  src: string;
  sandbox: string;
  csp: string;
  allow: string;
  referrerPolicy: 'strict-origin';
}

/** Features a frame never gets (permissions policy). */
const DENIED_FEATURES = [
  'camera',
  'microphone',
  'geolocation',
  'clipboard-read',
  'display-capture',
  'usb',
  'serial',
  'hid',
  'bluetooth',
  'payment',
];

/**
 * Attributes of the frame of `manifest`. `editorOrigin` is where the add-in
 * SDK script (`/addin-sdk/v1.js`) is served from.
 */
export function frameAttributes(
  manifest: AddinManifest,
  trusted: boolean,
  editorOrigin: string,
): FrameAttributes {
  const own = manifest.allowedOrigins.join(' ');
  const sdk = /^https?:\/\//.test(editorOrigin) ? ` ${editorOrigin}` : '';
  const network = networkOrigins(manifest.permissions);
  const csp = [
    `default-src ${own}`,
    `script-src ${own}${sdk}`,
    `style-src ${own} 'unsafe-inline'`,
    `img-src ${own} data: blob:`,
    `font-src ${own} data:`,
    `connect-src ${network.length ? network.join(' ') : "'none'"}`,
    "frame-src 'none'",
    "object-src 'none'",
    "base-uri 'none'",
    "form-action 'none'",
  ].join('; ');
  return {
    src: manifest.startUrl,
    sandbox: trusted ? 'allow-scripts allow-forms allow-same-origin' : 'allow-scripts allow-forms',
    csp,
    allow: DENIED_FEATURES.map((f) => `${f} 'none'`).join('; '),
    referrerPolicy: 'strict-origin',
  };
}
