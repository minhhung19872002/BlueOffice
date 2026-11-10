/**
 * Script-tag build of the client (`<script src="https://<blueoffice>/addin-sdk/v1.js">`):
 * the web app's build emits this file as `addin-sdk/v1.js` (IIFE, global
 * `BlueOfficeAddin`), so add-in pages need no bundler.
 */
export { AddinClient, AddinError, connect } from './client';
export { ERROR_CODES, PROTOCOL_VERSIONS } from './protocol';
export { validateManifest } from './manifest';
