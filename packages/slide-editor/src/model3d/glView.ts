/**
 * Minimal WebGL viewer for a 3D model while it is turned or panned (CORE-3DMODEL-001): one
 * vertex buffer, per-vertex colour, the camera-fixed key / fill / ambient lighting of the
 * engine's software renderer (`bo_model3d::raster`). The saved picture is rendered by the
 * engine when the drag ends.
 */
import type { Model3dView } from '@blueoffice/platform';
import { clipMatrix, lights, normalMatrix } from './view';

const VS = `
attribute vec3 aPos;
attribute vec3 aNormal;
attribute vec4 aColor;
uniform mat4 uClip;
uniform mat3 uNormal;
varying vec3 vNormal;
varying vec4 vColor;
void main() {
  gl_Position = uClip * vec4(aPos, 1.0);
  vNormal = uNormal * aNormal;
  vColor = aColor;
}`;

const FS = `
precision mediump float;
varying vec3 vNormal;
varying vec4 vColor;
uniform vec3 uKey;
uniform vec3 uFill;
uniform vec3 uEye;
void main() {
  vec3 n = normalize(vNormal);
  if (dot(n, uEye) < 0.0) n = -n;
  float light = 0.30 + 0.62 * max(dot(n, uKey), 0.0) + 0.22 * max(dot(n, uFill), 0.0);
  vec3 lin = pow(vColor.rgb, vec3(2.2)) * light;
  gl_FragColor = vec4(pow(clamp(lin, 0.0, 1.0), vec3(1.0 / 2.2)), 1.0);
}`;

export interface Mesh {
  positions: Float32Array;
  normals: Float32Array;
  colors: Uint8Array;
  indices: Uint32Array;
}

type GL = WebGLRenderingContext | WebGL2RenderingContext;

export class GlModelView {
  private constructor(
    private readonly gl: GL,
    private readonly prog: WebGLProgram,
    private readonly count: number,
    private readonly indexType: number,
  ) {}

  /** A viewer on `canvas`, or null without WebGL (the editor then shows the picture only). */
  static create(canvas: HTMLCanvasElement, mesh: Mesh): GlModelView | null {
    const opts = { alpha: true, antialias: true, premultipliedAlpha: false };
    const gl2 = canvas.getContext('webgl2', opts) as WebGL2RenderingContext | null;
    const gl: GL | null = gl2 ?? (canvas.getContext('webgl', opts) as WebGLRenderingContext | null);
    if (!gl) return null;
    const uint = gl2 !== null || gl.getExtension('OES_element_index_uint') !== null;
    const shader = (type: number, src: string) => {
      const s = gl.createShader(type);
      if (!s) return null;
      gl.shaderSource(s, src);
      gl.compileShader(s);
      return gl.getShaderParameter(s, gl.COMPILE_STATUS) ? s : null;
    };
    const vs = shader(gl.VERTEX_SHADER, VS);
    const fs = shader(gl.FRAGMENT_SHADER, FS);
    const prog = gl.createProgram();
    if (!vs || !fs || !prog) return null;
    gl.attachShader(prog, vs);
    gl.attachShader(prog, fs);
    gl.linkProgram(prog);
    if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) return null;
    gl.useProgram(prog);
    const attr = (
      name: string,
      data: ArrayBufferView,
      size: number,
      type: number,
      norm: boolean,
    ) => {
      const loc = gl.getAttribLocation(prog, name);
      const buf = gl.createBuffer();
      if (loc < 0 || !buf) return;
      gl.bindBuffer(gl.ARRAY_BUFFER, buf);
      gl.bufferData(gl.ARRAY_BUFFER, data, gl.STATIC_DRAW);
      gl.enableVertexAttribArray(loc);
      gl.vertexAttribPointer(loc, size, type, norm, 0, 0);
    };
    attr('aPos', mesh.positions, 3, gl.FLOAT, false);
    attr('aNormal', mesh.normals, 3, gl.FLOAT, false);
    attr('aColor', mesh.colors, 4, gl.UNSIGNED_BYTE, true);
    const ib = gl.createBuffer();
    if (!ib) return null;
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, ib);
    // Without 32-bit indices only meshes under 65 536 vertices can be drawn.
    if (!uint && mesh.positions.length / 3 > 65_535) return null;
    gl.bufferData(
      gl.ELEMENT_ARRAY_BUFFER,
      uint ? mesh.indices : Uint16Array.from(mesh.indices),
      gl.STATIC_DRAW,
    );
    gl.enable(gl.DEPTH_TEST);
    return new GlModelView(
      gl,
      prog,
      mesh.indices.length,
      uint ? gl.UNSIGNED_INT : gl.UNSIGNED_SHORT,
    );
  }

  /** Draw `view` for a frame of `frame` EMU into the whole canvas. */
  draw(view: Model3dView, frame: [number, number]): void {
    const { gl, prog } = this;
    gl.viewport(0, 0, gl.drawingBufferWidth, gl.drawingBufferHeight);
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    gl.useProgram(prog);
    gl.uniformMatrix4fv(
      gl.getUniformLocation(prog, 'uClip'),
      false,
      clipMatrix(view, frame[0], frame[1]),
    );
    gl.uniformMatrix3fv(gl.getUniformLocation(prog, 'uNormal'), false, normalMatrix(view));
    const l = lights(view);
    gl.uniform3fv(gl.getUniformLocation(prog, 'uKey'), l.key);
    gl.uniform3fv(gl.getUniformLocation(prog, 'uFill'), l.fill);
    gl.uniform3fv(gl.getUniformLocation(prog, 'uEye'), l.eye);
    gl.drawElements(gl.TRIANGLES, this.count, this.indexType, 0);
  }
}
