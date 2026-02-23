export class RatzillaCanvas {
    constructor() {}

    create_canvas_in_element(parent, width, height) {
        this.canvas = document.createElement("canvas");
        this.canvas.width = width
        this.canvas.height = height
        parent.appendChild(this.canvas);
    }

    init_ctx() {
        this.ctx = this.canvas.getContext("2d", {
            desynchronized: true,
            alpha: true
        });
        this.ctx.font = "16px monospace";
        this.ctx.textBaseline = "top";
    }

    share_ctx_with_other(other) {
      this.ctx = other.ctx;
      this.canvas = other.canvas;
    }

    get_canvas() {
        return this.canvas;
    }
}

