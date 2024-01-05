'use strict';

const SCREEN_DISTANCE = 280;
const WALL_HEIGHT = 2;
const SPEED = 0.003;
const ROTATE_SPEED = 0.003;
const canvas = document.getElementById('canvas');
const context = canvas.getContext('2d');

const main = () => {
    const now = new Date().getTime();
    const dt = now - lastUpdate;
    lastUpdate = now;
//    document.getElementById('lockMessage').innerText = dt;

    const {newX, newY} = updateMovement(dt);

    if (!crossRegion(playerX, playerY, playerX, newY)) {
        playerY = newY;
    }

    if (!crossRegion(playerX, playerY, newX, playerY)) {
        playerX = newX;
    }

    context.clearRect(0, 0, canvas.width, canvas.height);
    drawGrid();
    drawObjects();
    
    window.requestAnimationFrame(main);
};

const crossRegion = (playerX, playerY, newX, newY) => {
    return obstacles.some(o => o.cross(playerX, playerY, newX, newY));
};

const updateMovement = (dt) => {
    let newX = playerX;
    let newY = playerY;

    if (keysDown['w'] || keysDown[','] || touchMove === 'forward') {
        newX += SPEED*dt*Math.cos(theta);
        newY -= SPEED*dt*Math.sin(theta);
	}

	if (keysDown['s'] || keysDown['o'] || touchMove === 'backward') {
        newX -= SPEED*dt*Math.cos(theta);
        newY += SPEED*dt*Math.sin(theta);
	}

	if (keysDown['a']) {
        newX += SPEED*dt*Math.sin(theta);
        newY += SPEED*dt*Math.cos(theta);
	}
    
	if (keysDown['d'] || keysDown['e']) {
        newX -= SPEED*dt*Math.sin(theta);
        newY -= SPEED*dt*Math.cos(theta);
	}

	if (keysDown['Left'] || keysDown['ArrowLeft'] || touchMove === 'left') {
        theta -= ROTATE_SPEED*dt;
	}

	if (keysDown['Right'] || keysDown['ArrowRight'] || touchMove === 'right') {
        theta += ROTATE_SPEED*dt;
	}

    return {newX:newX,newY:newY};
};

const drawGrid = () => {


};

const drawObjects = () => {
	for (let screenX = 0; screenX < canvas.width; screenX++) {
        const ray = Math.atan2(screenX - canvas.width/2, SCREEN_DISTANCE);
        const checkAngle = theta + ray;
        const cos = Math.cos(checkAngle);
        const sin = Math.sin(checkAngle);

        const int = obstacles
            .map(o => o.intersect(playerX, playerY, cos, sin))
            .filter(i => i.distance > 0)
            .reduce(Intersection.min, Intersection.horizon);

        const wallStuff = SCREEN_DISTANCE*WALL_HEIGHT / Math.cos(ray) / int.distance;

        context.fillStyle = int.style;
        context.fillRect(screenX, canvas.height/2 - wallStuff, 1, 2*wallStuff);
	}
};

const setCanvasWidth = () => {
    canvas.width = window.innerWidth;
    canvas.height = window.innerHeight;
};

class Intersection {
    constructor(distance, color) {
        this.distance = distance;
        this.color = color;
    }
    
    static horizon = new this(Infinity, {r:0,g:0,b:0});

    static min(int0, int1) {
        return int0.distance < int1.distance? int0 : int1;
    }

    get style() {
        const f = 1 - Math.max(0.1, Math.min(0.7, this.distance/10));
        return `rgb(${f*this.color.r},${f*this.color.g},${f*this.color.b})`;
    }
}

class Figure {
    constructor(color) {
        this.color = color;
    }

    intersection(valid, distance) {
        return new Intersection(valid? distance : -1, this.color);
    }
}

class InsideCircle extends Figure {
    constructor(h, k, r, color) {
        super(color);
        this.h = h;
        this.k = k;
        this.r = r;
    }
    
    intersect(x0, y0, cos, sin) {
        let b = this.k*sin - this.h*cos + x0*cos - y0*sin;
        let descriminant = b*b - x0*x0 - y0*y0 + 2*this.h*x0 + 2*this.k*y0 - this.k*this.k - this.h*this.h + this.r*this.r;
        return this.intersection(descriminant > 0, -b + Math.sqrt(descriminant));  
    }

    #inside(x, y) {
        const dx = x - this.h;
        const dy = y - this.k;
        return dx*dx + dy*dy < this.r*this.r;
    }

    cross(x0, y0, x1, y1) {
        return this.#inside(x0, y0) !== this.#inside(x1, y1);
    }
}

class OutsideCircle extends Figure {
    constructor(h, k, r, color) {
        super(color);
        this.h = h;
        this.k = k;
        this.r = r;
    }
    
    intersect(x0, y0, cos, sin) {
        let b = this.k*sin - this.h*cos + x0*cos - y0*sin;
        let descriminant = b*b - x0*x0 - y0*y0 + 2*this.h*x0 + 2*this.k*y0 - this.k*this.k - this.h*this.h + this.r*this.r;
        return this.intersection(descriminant > 0, -b - Math.sqrt(descriminant));  
    }

    #inside(x, y) {
        const dx = x - this.h;
        const dy = y - this.k;
        return dx*dx + dy*dy < this.r*this.r;
    }

    cross(x0, y0, x1, y1) {
        return this.#inside(x0, y0) !== this.#inside(x1, y1);
    }
}

class HorizontalLine extends Figure {
    constructor(lineY, xStart, xEnd, color) {
        super(color);
        this.lineY = lineY;
        this.xStart = xStart;
        this.xEnd = xEnd;
    }

    intersect(x0, y0, cos, sin) {
        let dist = (y0-this.lineY)/sin;
        let x = x0 + dist*cos;
        return this.intersection(x<=this.xEnd && x>=this.xStart, dist);
    }

    #inBound(x) {
        return x >= this.xStart && x <= this.xEnd;
    }

    cross(x0, y0, x1, y1) {
        return this.#inBound(x0) && this.#inBound(x1) && (y0 < this.lineY) !== (y1 < this.lineY);
    }
}

class VerticalLine extends Figure {
    constructor(lineX, yStart, yEnd, color) {
        super(color);
        this.lineX = lineX;
        this.yStart = yStart;
        this.yEnd = yEnd;
    }

    intersect(x0, y0, cos, sin) {
        let dist = (this.lineX - x0)/cos;
        let y = y0 - dist*sin;
        return this.intersection(y<=this.yEnd && y>=this.yStart, dist);
    }

    cross(x0, y0, x1, y1) {
        return this.#inBound(y0) && this.#inBound(y1) && (x0 < this.lineX) !== (x1 < this.lineX);
    }

    #inBound(y) {
        return y >= this.yStart && y <= this.yEnd;
    }
}

let theta = 0;
let playerX = -5;
let playerY = 3;
let lastUpdate = new Date().getTime();
let keysDown = {};
let obstacles = [
    new HorizontalLine(0, 0, 9, {r:255,g:255,b:0}),
    new HorizontalLine(9, 0, 9, {r:255,g:0,b:0}),
    new VerticalLine(0, 0, 9, {r:255,g:0,b:255}),
    new VerticalLine(9, 0, 9, {r:0,g:255,b:0}),
    new OutsideCircle(5, 5, 0.5, {r:0,g:255,b:255})
]; 
var touchMove = 'none';
document.addEventListener('touchstart', event => {
    event.preventDefault();
    let x = event.touches[0].clientX;
    let y = event.touches[0].clientY;

    if (y < canvas.height/5) {
        touchMove = 'forward';
    } else if (y > 4*canvas.height/5) {
        touchMove = 'backward'
    } else {
        if (x < canvas.width/2) {
            touchMove = 'left';
        } else {
           touchMove = 'right';
        }
    }
});
document.addEventListener('touchend', event => touchMove = 'none');
document.addEventListener('mousedown', event => {
    canvas.requestPointerLock();
});
document.addEventListener('keydown', event => {
    keysDown[event.key] = true;
});
document.addEventListener('keyup', event => keysDown[event.key] = false);
document.addEventListener('resize', setCanvasWidth);
document.addEventListener('mousemove', event => {
    if (event.movementX) {
        theta += 0.005 * event.movementX;
    }
});

document.addEventListener('pointerlockchange', event => {
    if (document.pointerLockElement === canvas) {
//        document.getElementById('lockMessage').style.display = 'none';
    } else {
//        document.getElementById('lockMessage').style.display = '';
    }
});

setCanvasWidth();
main();

