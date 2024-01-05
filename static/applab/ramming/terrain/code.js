var TAU = 2*Math.PI;
var COLOR_CLEAR = rgb(0, 0, 0, 0);

var SCREEN_WIDTH = 320;
var SCREEN_HEIGHT = 450;
var GRID_SQUARE_WIDTH = 20;



var backgroundIndex = 1;


setActiveCanvas("canvas");
setFillColor("red");
setStrokeColor(COLOR_CLEAR);



var client = new Client();




function Client() {
  this.eventClickBoatSprite = function (localPos, boat) {
    // we want to spawn a new 
    if (this.boatClickCrosshair) return;
    this.boat = boat;
    this.boatClickCrosshair = this.sprites.addSprite(CrosshairSprite, localPos[0], localPos[1]);
  }; 
  
  this.randomSeed = randomNumber(1000, 100000);
  this.sprites = new Sprites(this.randomSeed);
  this.sprites.addBackground();
  
  this.sprites.addSprite(BoatSprite, 100, 200);
  this.sprites.addSprite(BoatSprite, 200, 200);
  this.sprites.addSprite(CarrierSprite, 180, 260);
  
  
  var that = this;
  onEvent("main", "mousedown", function (event) {
    if (that.boatClickCrosshair) {
      that.sprites.deleteSprite(that.boatClickCrosshair.id);
      that.boatClickCrosshair = null;
      var endPos = that.sprites.map.fromScreen(event.x, event.y);
      
      that.boat.sailTo(endPos);
    }
  });
  
  onEvent("main", "mousemove", function (event) {
    if (that.boatClickCrosshair) {
      // we are tracking the 
      var local = that.sprites.map.fromScreen(event.x, event.y);
      that.boatClickCrosshair.setLocation(local[0], local[1]);
    }
  });
}


function Sprites(seed) {
  //  manipulating sprites and everything
  // includes both the grid and the sprite
  
  this.checkBackgroundCollisions = function () {
    var collide = [];
    for (var i=0; i<sprites.length; i++) {
      if (sprites[i] !== null && sprites[i].collidesWithBackground()) {
        collide.push(sprites[i]);
      }
    }
    
    return collide;
  };

  this.addBackground = function () {
    sprites.push(new BackgroundSprite(160, 225, this.map, "0"));
  };

  this.checkSpriteCollisions = function () {
    // returns a list of the sprites that collide
    var collide = [];
    
    // iterate through all pairs of objects
    for (var inc=1; inc < sprites.length; inc++) {
      for (var start=0; start+inc < sprites.length; start++) {
        var
          a = sprites[start],
          b = sprites[start+inc];
          
        if (a !== null && b !== null && a.collidesWithOther(b)) {
          collide.push(a);
          collide.push(b);
        }
      }
    }
    
    return collide;
  };

  this.deleteSprite = function (id) {
    var index = parseInt(id);
    sprites[index].delete();
    sprites[index] = null;
  };

  this.addSprite = function (constructor, x, y) {
    var id = sprites.length.toString();
    var sprite = new constructor(x, y, this.map, id);
    sprites.push(sprite);
    return sprite;
  };
  
  this.resizeUpdate = function () {
    // redraws the sprites and the grid
    this.map = new Map(this.topLeftX, this.topLeftY, this.scaleFactor);
    for (var i=0; i<sprites.length; i++) {
      if (sprites[i] !== null)
        sprites[i].setMap(this.map);
    }
  };
  
  var sprites = []; // may contain nulls
  this.map = new Map(0, 0, 1);

  var that = this;
  
  this.topLeftX = 0;
  this.topLeftY = 0;
  this.scaleFactor = 1;
  
  /*
  var lastX = null;
  var lastY = null;
  
  onEvent("main", "mousedown", function(event) {
    lastX = event.x - that.topLeftX;
    lastY = event.y - that.topLeftY;
  });
  
  onEvent("main", "mouseup", function() { lastX = null; });
  onEvent("main", "mouseout", function() { lastX = null; });
  
  onEvent("main", "mousemove", function(event) {
    if (lastX) {
      that.topLeftX = event.x-lastX;
      that.topLeftY = event.y-lastY;
      
      that.resizeUpdate();
    }
  });
  */
  
  onEvent("main", "keydown", function(event) {
    if (event.key == "Up" | event.key == "Down") {
      var f = (event.key=="Up")? 1.1 : 0.9;
      
      var dx = 160 - that.topLeftX;
      var dy = 225 - that.topLeftY;
      
      dx *= f;
      dy *= f;
      that.scaleFactor *= f;
      
      that.topLeftX = 160-dx;
      that.topLeftY = 225-dy;
      
      that.resizeUpdate();
    } else if ("wasd,oe".includes(event.key)) { // works with qwerty or dvorak
      var shift;
      if ("w,".includes(event.key)) {
        shift = [0, 10];
      } else if ("so".includes(event.key)) {
        shift = [0, -10];
      } else if ("a" == event.key) {
        shift = [10, 0]; 
      } else { // e or d
        shift = [-10, 0];
      }
      
      that.topLeftX += shift[0];
      that.topLeftY += shift[1];
      
      that.resizeUpdate();
    }
  });
}



function Map(cornerX, cornerY, scaleFactor) {
  this.toScreen = function (oldX, oldY, oldWidth, oldHeight) {
    var newX = cornerX + scaleFactor*oldX;
    var newY = cornerY + scaleFactor*oldY;
    
    return [newX, newY, oldWidth*scaleFactor, oldHeight*scaleFactor];
  };
  
  this.fromScreen = function (screenX, screenY) {
    // return it to the background coordinates
    return [
      (screenX - cornerX)/ scaleFactor,
      (screenY - cornerY)/ scaleFactor,
    ];
  };
}

function CarrierSprite(x, y, map, id) {
  var hitbox = hitboxFrom("carrier", x, y);
  Sprite.call(this, x, y, 120, 20, map, id, hitbox, "carrier.png");

  
  var that = this;

  this.sailTo = function(pos) {
    var xDistance = pos[0] - that.x;
    var yDistance = pos[1] - that.y;
  
    that.rotateToAnd(Math.atan2(yDistance, xDistance), function(actualAngle) {
      // this is what we do after we rotate
      that.sailHelper(pos, actualAngle);
    });
  };

  this.rotateToAnd = function (newAngle, callback) {
    var currentAngle = that.angle;
    var angleDistance = newAngle - currentAngle;
    // we want to move in the direction that requires the least movement
    var option1 = angleDistance%TAU; // going left
    var option2 = TAU-option1; // going right

    var change = (Math.abs(option1) < Math.abs(option2)) ? option1 : option2;
    var stepsLeft = Math.ceil(Math.abs(change) / 0.1); // 0.1 is our step size

    var dTheta = 0.1*sign(change);

    var loopHandle = timedLoop(50, function() {
    
      that.setAngle(currentAngle+dTheta);
      
      // TODO: add rotation collision detection
      if (that.collidesWithBackground()) {
        // end the loop here too
        that.setAngle(currentAngle);
        
        stopTimedLoop(loopHandle);
        return;
      }
      
      
      currentAngle += dTheta;
      
      stepsLeft--;
      if (stepsLeft <= 0) {
        // we are done
        stopTimedLoop(loopHandle);
        callback(currentAngle);
      }
    });
  };
  
  this.sailHelper = function(pos, actualAngle) {
    var currentPosX = that.x;
    var currentPosY = that.y;
    
    var dx = Math.cos(actualAngle);
    var dy = Math.sin(actualAngle);
 
    var distanceToSail = distance(currentPosX, currentPosY, pos[0], pos[1]);
    var stepsLeft = distanceToSail/1; // because [dx, dy] is a unit vector
 
    var loopHandle = timedLoop(50, function() {
      // check if it is legal to move there
      that.setLocation(currentPosX + dx, currentPosY + dy);
      if (that.collidesWithBackground()) {
        stopTimedLoop(loopHandle);
        that.setLocation(currentPosX, currentPosY); // move it backards
        return;
      }
      
      currentPosX += dx;
      currentPosY += dy;

      // check if the boat crashed with the shoreline
      stepsLeft--;
      if (stepsLeft <= 0) {
        stopTimedLoop(loopHandle);
      }
    });
  };
  

  
  onEvent(this.id, "click", function (event) {
    var localPos = that.map.fromScreen(event.x, event.y);
    
    client.eventClickBoatSprite(localPos, that);
  });
}



function BoatSprite(x, y, map, id) {
  var hitbox = hitboxFrom("square", x, y);
  Sprite.call(this, x, y, 20*Math.sqrt(2), 20*Math.sqrt(2), map, id, hitbox, "square.png");
  
  var that = this;

  this.sailTo = function(pos) {
    var xDistance = pos[0] - that.x;
    var yDistance = pos[1] - that.y;
  
    that.rotateToAnd(Math.atan2(yDistance, xDistance), function(actualAngle) {
      // this is what we do after we rotate
      that.sailHelper(pos, actualAngle);
    });
  };

  this.rotateToAnd = function (newAngle, callback) {
    var currentAngle = that.angle;
    var angleDistance = newAngle - currentAngle;
    // we want to move in the direction that requires the least movement
    var option1 = angleDistance%TAU; // going left
    var option2 = TAU-option1; // going right

    var change = (Math.abs(option1) < Math.abs(option2)) ? option1 : option2;
    var stepsLeft = Math.ceil(Math.abs(change) / 0.1); // 0.1 is our step size

    var dTheta = 0.1*sign(change);

    var loopHandle = timedLoop(50, function() {
    
      that.setAngle(currentAngle+dTheta);
      
      // TODO: add rotation collision detection
      if (that.collidesWithBackground()) {
        // end the loop here too
        that.setAngle(currentAngle);
        
        stopTimedLoop(loopHandle);
        return;
      }
      
      
      currentAngle += dTheta;
      
      stepsLeft--;
      if (stepsLeft <= 0) {
        // we are done
        stopTimedLoop(loopHandle);
        callback(currentAngle);
      }
    });
  };
  
  this.sailHelper = function(pos, actualAngle) {
    var currentPosX = that.x;
    var currentPosY = that.y;
    
    var dx = Math.cos(actualAngle);
    var dy = Math.sin(actualAngle);
 
    var distanceToSail = distance(currentPosX, currentPosY, pos[0], pos[1]);
    var stepsLeft = distanceToSail/1; // because [dx, dy] is a unit vector
 
    var loopHandle = timedLoop(50, function() {
      // check if it is legal to move there
      that.setLocation(currentPosX + dx, currentPosY + dy);
      if (that.collidesWithBackground()) {
        stopTimedLoop(loopHandle);
        that.setLocation(currentPosX, currentPosY); // move it backards
        return;
      }
      
      currentPosX += dx;
      currentPosY += dy;

      // check if the boat crashed with the shoreline
      stepsLeft--;
      if (stepsLeft <= 0) {
        stopTimedLoop(loopHandle);
      }
    });
  };
  

  
  onEvent(this.id, "click", function (event) {
    var localPos = that.map.fromScreen(event.x, event.y);
    
    client.eventClickBoatSprite(localPos, that);
  });
}


function CrosshairSprite(x, y, map, id) {
  var hitbox = hitboxFrom("none", x, y);
  Sprite.call(this, x, y, 45, 45, map, id, hitbox, "crosshair.png");
}

function SquareSprite(x, y, map, id) {
  var hitbox = hitboxFrom("square", x, y);
  
  Sprite.call(this, x, y, 28.28, 28.28, map, id, hitbox, "square.png");
  
  //var that = this;
  //onEvent(this.id, "click", function(event) {
  //  var local = that.map.fromScreen(event.x, event.y);
  //  client.eventSelectSquareSprite(local);
  //});
}

function BackgroundSprite(x, y, map, id) {
  var name = "background"+backgroundIndex+".png";
  var hitbox = hitboxFrom("none", x, y);
  Sprite.call(this, x, y, 320, 450, map, id, hitbox, name);
  
  
}


function Sprite(x, y, width, height, map, id, hitbox, imageName) {
  this.collidesWithBackground = function () {
    return this.hitbox.collidesWithBackground();
  };
  
  this.collidesWithOther = function(other) {
    return this.hitbox.collidesWithOther(other);
  };
  
  this.setMap = function(newMap) {
    this.map = newMap;
    this.resizeUpdate();
  };
  
  this.setAngle = function(newAngle) {
    this.angle = newAngle;
    setStyle(this.id, "transform: rotate("+ (this.angle) +"rad);");
    this.hitbox.setAngle(this.angle);
  };
  
  this.getAngle = function () {
    return this.angle;
  };
  
  this.setLocation = function(newX, newY) {
    this.x = newX;
    this.y = newY;
    this.resizeUpdate();
    this.hitbox.setLocation(this.x, this.y);
  };

  this.resizeUpdate = function() {
    // draws the sprite
    var mapped = this.map.toScreen(this.x, this.y, this.width, this.height);
    var displayWidth = mapped[2];
    var displayHeight = mapped[3];
    setPosition(id, mapped[0]- displayWidth/2, mapped[1]-displayHeight/2, displayWidth, displayHeight);
  };
  
  this.delete = function() {
    this.hitbox = null;
    deleteElement(this.id);
  };
  
  this.x = x; // coordinates of the center
  this.y = y;
  this.width = width;
  this.height = height;
  
  this.angle = 0;
  this.map = map;
  this.hitbox = hitbox;
  this.id = id;
  
  button(this.id, "");
  setProperty(this.id, "background-color", COLOR_CLEAR);
  setProperty(this.id, "image", imageName);
  setStyle(this.id, "padding: 0px;");

  this.resizeUpdate();
}







function LineSegment(x0, y0, x1, y1) {
  if (x0 == x1) x0 += 0.1;
  
  this.slope = (y1-y0)/(x1-x0);
  this.y_int = y0-this.slope*x0;
  var xMin = Math.min(x0, x1);
  var xMax = Math.max(x0, x1);
  var yMin = Math.min(y0, y1);
  var yMax = Math.max(y0, y1);

  this.includes_x = function(x) {
    return xMin <= x && x <= xMax;
  };
  
  this.includes_y = function(y) {
    return yMin <= y && y <= yMax;
  };

  this.intersectWithOther = function(other) {
    var x = (this.y_int - other.y_int)/(other.slope - this.slope);
    var y = this.slope*x+this.y_int;
    return this.includes_x(x) && other.includes_x(x) && this.includes_y(y) && other.includes_y(y);
  };
}


function hitboxFrom(kind, x, y) {
  var vertices;
  
  if (kind === "square") {
    vertices = [
      [20, TAU/8],
      [20, 7*TAU/8],
      [20, 5*TAU/8],
      [20, 3*TAU/8],
    ];
    
  } else if (kind === "carrier") {
    vertices = [
      [59, 0.052],
      [9.8, 1.57],
      [58, 3.05],
      [58, 3.23],
      [10, -1.57],
      [59, -0.052],
    ];
    
  } else if (kind === "none") {
    vertices = [];
  } else {
    throw "no hitbox called "+kind;
  }
  
  return new Hitbox(x, y, vertices);
}


function Hitbox(x, y, vertices) {
  this.setAngle = function (newAngle) {
    angle = newAngle;
    this.lines = this.getSlopeInterceptLines();
  };

  this.setLocation = function (newX, newY) {
    this.x = newX;
    this.y = newY;
    this.lines = this.getSlopeInterceptLines();
  };

  this.getPoint = function (i) {
    var 
      index = (i<this.vertices.length)? i : 0,
      vertex = this.vertices[index],
      r = vertex[0],
      newAngle = vertex[1]+angle;
      
    return [
      this.x+r*Math.cos(newAngle), 
      this.y+r*Math.sin(newAngle),
    ];
  };
  
  this.getSlopeInterceptLines = function () {
    var lines = [];
    for (var i=0; i<this.vertices.length; i++) {
      var
        start = this.getPoint(i),
        end = this.getPoint(i+1),
        lineSegment = new LineSegment(start[0], start[1], end[0], end[1]);
        
      lines.push(lineSegment);
    }
    return lines;
  };
   
  this.collidesWithOther = function (other) {
    if (distance(this.x, this.y, other.x, other.y) < this.maxRadius + other.maxRadius) {
      // they might collide, lets do a more in-depth check to see
      var thisLines = this.lines();
      var otherLines = other.lines();
      
      for (var i=0; i<thisLines.length; i++) {
        for (var j=0; j<otherLines.length; j++) {
          if (thisLines[i].intersectWithOther(otherLines[j])) {
            return true;
          }
        }
      }
      
      return false;
    } else {
      return false;
    }
  };
  
  this.collidesWithBackground = function () {
    setStyle("canvas", "z-index: 999;");

    for (var i=0; i<this.vertices.length; i++) {
      var 
        start = this.getPoint(i),
        x0 = Math.round(start[0]),
        y0 = Math.round(start[1]),
        end = this.getPoint(i+1),
        x1 = Math.round(end[0]),
        y1 = Math.round(end[1]);
        
      var doesntHitShore = true;
        
      plotLine(x0, y0, x1, y1, function(plotX, plotY) {
        //clearCanvas("canvas");
        rect(plotX, plotY, 1, 1);
        if (isSolid(plotX, plotY)) {
          doesntHitShore = false;
          return;
        }
      });
      
      if (!doesntHitShore) return true;
    }
    
    return false;
  };
  
  
  var angle = 0;
  this.x = x;
  this.y = y;
  this.vertices = vertices;
  this.lines = this.getSlopeInterceptLines();
  
  this.maxRadius = 0;
  for (var i=0; i<this.vertices.length; i++) {
    this.maxRadius = Math.max(this.maxRadius, vertices[i][0]);
  }
}


function isSolid(x, y) {
  var i = x + y*320;
  var numberIndex = (i/32)|0;
  var numberShift = i % 32;
  
  var number = MAPS[backgroundIndex][numberIndex];
  
  return (number >> numberShift) & 1;
}



function map(x, in_min, in_max, out_min, out_max) {
  // https://www.arduino.cc/reference/en/language/functions/math/map/
  return (x - in_min) * (out_max - out_min) / (in_max - in_min) + out_min;
}


function distance(x0, y0, x1, y1) {
  var dx = x1-x0;
  var dy = y1-y0;
  return Math.sqrt(dx*dx + dy*dy);
}


function sign(n) {
  return n < 0 ? -1 : 1;
}

function uniqueId() {
  // kill me later for this
  return randomNumber(0, 999999999).toString();
}

function plotLine(x0, y0, x1, y1, plot) {
    x0 |= 0;
    y0 |= 0;
    x1 |= 0;
    y1 |= 0;
    
    // stolen directly from wikipedia's bresenham algorithm page
    
    if (Math.abs(y1 - y0) < Math.abs(x1 - x0)) {
        if (x0 > x1) {
            plotLineLow(x1, y1, x0, y0, plot);
        }  else {
            plotLineLow(x0, y0, x1, y1, plot);
        }
        
    } else {
        if (y0 > y1) {
            plotLineHigh(x1, y1, x0, y0, plot);
        } else {
            plotLineHigh(x0, y0, x1, y1, plot);
        }
        
    }
}

function plotLineHigh(x0, y0, x1, y1, plot) {
    var dx = x1 - x0;
    var dy = y1 - y0;
    
    var xi = 1;
    if (dx < 0) {
        xi = -1;
        dx = -dx;
    }
    
    var D = 2*dx - dy;
    var x = x0;
    
    for (var y=y0; y<=y1; y++) {
        plot(x, y);
        if (D > 0) {
            x += xi;
            D -= 2*dy;
        }
        
        D += 2*dx;
    }

}

// taken from wikipedia bresenham's article
function plotLineLow(x0, y0, x1, y1, plot) {
    var dx = x1 - x0;
    var dy = y1 - y0;
    
    var yi = 1;
    
    if (dy < 0) {
        yi = -1;
        dy = -dy;
    }
    
    var D = 2*dy - dx;
    var y = y0;
    
    for (var x=x0; x<=x1; x++) {
        plot(x, y);
        
        if (D > 0) {
            y += yi;
            D = D - 2*dx;
        }
        
        D += 2*dy;
    }
}


/*
function plotLine(x0, y0, x1, y1, plot) {
  // calls plot on all of the points that goes through the line described here
  
  var dx = x1 - x0;
  var dy = y1 - y0;
  
  var lineX, lineY, yMin, yMax, xMin, xMax;
      
  if (dx !== 0) {
    // the line is not vertical
    var slope = dy / dx;
    var yInt = y0 - slope*x0;
    
    if (Math.abs(dx) > Math.abs(dy)) {
      // the line is more horizontal, so we should draw the line through the x coords
      xMin = Math.min(x0, x1);
      xMax = Math.max(x0, x1);
      
      for (lineX=xMin; lineX<=xMax; lineX++) {
        lineY = slope*lineX + yInt;
        
        // plot the point
        plot(lineX|0, lineY|0);
      }
    } else {
      yMin = Math.min(y0, y1);
      yMax = Math.max(y0, y1);
      
      for (lineY=yMin; lineY<=yMax; lineY++) {
        lineX = (lineY-yInt)/slope;
        plot(lineX|0, lineY|0);
      }
    }
    // TODO: bresenham
    
  } else {
    // we have a vertical line
    yMin = Math.min(y0, y1);
    yMax = Math.max(y0, y1);
    
    lineX = x0;
    for (lineY=yMin; lineY<=yMax; lineY++) {
      plot(lineX|0, lineY|0);
    }
  }
}
*/

function randomColor() {
  return rgb(
    randomNumber(0,255),
    randomNumber(0,255),
    randomNumber(0,255)
  );
}

var MAPS = [
  [0,0,3758096384,4294967295,511,0,0,2147483648,4294967295,524287,0,0,3758096384,4294967295,1023,0,0,0,4294967295,524287,0,0,4026531840,4294967295,1023,0,0,0,4294967295,1048575,0,0,4026531840,4294967295,2047,0,0,0,4294967294,1048575,0,0,4026531840,4294967295,4095,0,0,0,4294967294,1048575,0,0,4026531840,4294967295,8191,0,0,0,4294967292,2097151,0,0,4026531840,4294967295,16383,0,0,0,4294967292,2097151,2031616,0,4026531840,4294967295,32767,0,0,0,4294967292,4194303,8372224,0,4160749568,4294967295,32767,0,0,0,4294967288,4194303,16769024,0,4160749568,4294967295,65535,0,0,0,4294967288,4194303,33550336,0,4160749568,4294967295,131071,0,0,0,4294967280,8388607,67106816,0,4160749568,4294967295,262143,0,0,0,4294967280,8388607,134216704,0,4160749568,4294967295,524287,0,0,0,4294967280,8388607,268434432,0,4227858432,4294967295,2097151,0,0,0,4294967264,16777215,268434944,0,4227858432,4294967295,4194303,0,0,0,4294967264,16777215,536870656,0,4227858432,4294967295,8388607,0,0,0,4294967264,16777215,1073741696,0,4227858432,4294967295,16777215,0,0,0,4294967232,16777215,2147483584,0,4261412864,4294967295,67108863,0,0,0,4294967232,16777215,4294967232,0,4261412864,4294967295,134217727,0,0,0,4294967232,33554431,4294967264,0,4261412864,4294967295,536870911,0,0,0,4294967168,33554431,4294967280,1,4278190080,4294967295,2147483647,0,0,0,4294967168,33554431,4294967280,3,4278190080,4294967295,4294967295,1,0,0,4294967168,33554431,4294967288,7,4278190080,4294967295,4294967295,7,0,0,4294967040,33554431,4294967292,7,4286578688,4294967295,4294967295,15,0,0,4294967040,33554431,4294967294,15,4286578688,4294967295,4294967295,31,0,0,4294966784,16777215,4294967295,31,4286578688,4294967295,4294967295,63,0,0,4294966784,16777215,4294967295,63,4290772992,4294967295,4294967295,127,0,0,4294966784,16777215,4294967295,127,4290772992,4294967295,4294967295,127,0,0,4294966272,16777215,4294967295,127,4292870144,4294967295,4294967295,127,0,0,4294966272,8388607,4294967295,255,4292870144,4294967295,4294967295,255,0,0,4294965248,8388607,4294967295,511,4293918720,4294967295,4294967295,255,0,0,4294965248,8388607,4294967295,1023,4293918720,4294967295,4294967295,255,0,0,4294963200,4194303,4294967295,2047,4294443008,4294967295,4294967295,255,0,0,4294963200,2097151,4294967295,4095,4294443008,4294967295,4294967295,255,0,0,4294959104,2097151,4294967295,8191,4294705152,4294967295,4294967295,255,0,0,4294950912,1048575,4294967295,16383,4294705152,4294967295,4294967295,255,0,0,4294950912,524287,4294967295,32767,4294836224,4294967295,4294967295,127,0,0,4294934528,262143,4294967295,65535,4294901760,4294967295,4294967295,127,0,0,4294901760,131071,4294967295,131071,4294934528,4294967295,4294967295,63,0,0,4294836224,65535,4294967295,524287,4294934528,4294967295,4294967295,63,0,0,4294705152,16383,4294967295,1048575,4294950912,4294967295,4294967295,31,0,0,4293918720,8191,4294967295,2097151,4294959104,4294967295,4294967295,31,0,0,4292870144,2047,4294967295,8388607,4294963200,4294967295,4294967295,15,0,0,4278190080,255,4294967295,33554431,4294966272,4294967295,4294967295,7,0,0,4026531840,7,4294967295,134217727,4294966784,4294967295,4294967295,3,0,0,0,0,4294967295,1073741823,4294967168,4294967295,4294967295,1,0,0,0,0,4294967295,4294967295,4294967295,4294967295,4294967295,1,0,0,0,0,4294967295,4294967295,4294967295,4294967295,4294967295,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,2147483647,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,1073741823,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,536870911,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,268435455,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,134217727,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,67108863,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,67108863,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,33554431,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,16777215,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,8388607,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,8388607,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,4194303,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,2097151,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,1048575,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,1048575,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,524287,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,262143,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,131071,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,131071,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,65535,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,32767,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,16383,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,16383,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,8191,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,4095,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,2047,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,1023,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,511,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,255,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,127,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,63,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,31,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,7,0,0,0,0,0,4294967295,4294967295,2147483647,4294901760,0,0,2147483648,15,0,0,4294967295,4294967295,268435455,0,0,0,3221225472,63,0,0,4294967295,4294967295,67108863,0,0,0,3758096384,63,0,0,4294967295,4294967295,16777215,0,0,0,4026531840,127,0,0,4294967295,4294967295,8388607,0,0,0,4160749568,255,0,0,4294967295,4294967295,4194303,0,0,0,4227858432,255,0,0,4294967295,4294967295,1048575,0,0,0,4227858432,255,0,0,4294967295,4294967295,524287,0,0,0,4227858432,255,0,0,4294967295,4294967295,262143,0,0,0,4261412864,255,0,0,4294967295,4294967295,131071,0,0,0,4261412864,511,0,0,4294967295,4294967295,65535,0,0,0,4261412864,255,0,0,4294967295,4294967295,32767,0,0,0,4261412864,255,0,0,4294967295,4294967295,16383,0,0,0,4261412864,255,0,0,4294967295,4294967295,16383,0,0,0,4261412864,255,0,0,4294967295,4294967295,8191,0,0,0,4261412864,255,0,0,4294967295,4294967295,4095,0,0,0,4261412864,127,0,0,4294967280,4294967295,2047,0,0,0,4227858432,127,0,0,4294967168,4294967295,1023,0,0,0,4227858432,63,0,0,4294966272,4294967295,511,0,0,0,4227858432,63,0,0,4294963200,4294967295,255,0,0,0,4160749568,31,0,0,4294950912,4294967295,255,0,0,0,4026531840,15,0,0,4294901760,4294967295,127,0,0,0,3758096384,7,0,0,4294836224,4294967295,63,0,0,0,0,0,0,0,4294443008,4294967295,31,0,0,0,0,0,0,0,4293918720,4294967295,15,0,0,0,0,0,0,0,4292870144,4294967295,7,0,0,0,0,0,0,0,4286578688,4294967295,3,0,0,0,0,0,0,0,4278190080,4294967295,3,0,0,0,0,0,0,0,4261412864,4294967295,1,0,0,0,0,0,0,0,4227858432,4294967295,0,0,0,0,0,0,0,0,4026531840,2147483647,0,0,0,0,0,0,0,0,3758096384,1073741823,0,0,0,0,0,0,0,0,3221225472,536870911,0,0,0,0,0,0,0,0,0,134217727,0,0,0,0,0,0,0,0,0,67108862,0,0,0,0,0,0,0,0,0,33554424,0,0,0,0,0,0,0,0,0,8388592,0,0,0,0,0,0,0,0,0,2097024,0,0,65024,0,0,0,0,0,0,523264,0,0,130816,0,0,0,0,0,0,0,0,0,524160,0,0,0,0,0,0,0,0,0,1048512,0,0,0,0,0,0,0,0,0,2097120,0,0,0,0,0,0,0,0,0,4194272,0,0,0,0,0,0,0,0,0,8388592,0,0,0,0,0,0,0,0,0,16777200,0,0,0,0,0,0,0,0,0,33554424,0,0,0,0,0,0,0,0,0,67108856,0,0,0,0,0,0,0,0,0,134217724,0,0,0,0,0,0,0,0,0,268435452,0,0,0,0,0,0,0,0,0,536870908,0,0,0,0,2147483648,0,0,0,0,1073741822,0,0,0,0,3758096384,0,0,0,0,2147483646,0,0,0,0,4160749568,0,0,0,0,2147483647,0,0,0,0,4227858432,0,0,0,0,4294967295,0,0,0,0,4278190080,0,0,0,0,4294967295,1,0,0,0,4286578688,0,0,0,2147483648,4294967295,3,0,0,0,4290772992,0,0,0,2147483648,4294967295,7,0,0,0,4292870144,0,0,0,2147483648,4294967295,15,0,0,0,4293918720,0,0,0,3221225472,4294967295,31,0,0,0,4294705152,0,0,0,3221225472,4294967295,63,0,0,0,4294705152,0,0,0,3221225472,4294967295,127,0,0,0,4294836224,0,0,0,3221225472,4294967295,255,0,0,0,4294901760,0,0,0,3758096384,4294967295,1023,0,0,0,4294934528,0,0,0,3758096384,4294967295,2047,0,0,0,4294950912,0,0,0,3758096384,4294967295,4095,0,0,0,4294959104,0,0,0,4026531840,4294967295,8191,0,0,0,4294959104,0,0,0,4026531840,4294967295,16383,0,0,0,4294963200,0,0,0,4026531840,4294967295,32767,0,0,0,4294965248,0,0,0,4026531840,4294967295,65535,0,0,0,4294965248,0,0,0,4160749568,4294967295,65535,0,0,0,4294966272,0,0,0,4160749568,4294967295,131071,0,0,0,4294966784,0,0,0,4160749568,4294967295,262143,0,0,0,4294966784,0,0,0,4160749568,4294967295,524287,0,0,0,4294967040,0,0,0,4227858432,4294967295,524287,0,0,0,4294967040,0,0,0,4227858432,4294967295,1048575,0,0,0,4294967168,0,0,0,4227858432,4294967295,1048575,0,0,0,4294967168,0,0,0,4227858432,4294967295,2097151,0,0,0,4294967232,0,0,0,4261412864,4294967295,2097151,0,0,0,4294967232,0,0,0,4261412864,4294967295,4194303,0,0,0,4294967264,0,0,0,4261412864,4294967295,4194303,0,0,0,4294967264,0,0,0,4261412864,4294967295,4194303,0,0,0,4294967280,0,0,0,4278190080,4294967295,8388607,0,0,0,4294967280,0,0,0,4278190080,4294967295,8388607,0,0,0,4294967288,0,0,0,4278190080,4294967295,8388607,0,0,0,4294967288,0,0,0,4286578688,4294967295,8388607,0,0,0,4294967288,0,0,0,4286578688,4294967295,8388607,0,0,0,4294967292,0,0,0,4286578688,4294967295,8388607,0,0,0,4294967292,0,0,0,4286578688,4294967295,8388607,0,0,0,4294967294,0,0,0,4290772992,4294967295,16777215,0,0,0,4294967294,0,0,0,4290772992,4294967295,16777215,0,0,0,4294967294,0,0,0,4290772992,4294967295,16777215,0,0,0,4294967295,0,0,0,4290772992,4294967295,16777215,0,0,0,4294967295,0,0,0,4292870144,4294967295,16777215,0,0,0,4294967295,0,0,0,4292870144,4294967295,8388607,0,0,2147483648,4294967295,0,0,0,4292870144,4294967295,8388607,0,0,2147483648,4294967295,0,0,0,4293918720,4294967295,8388607,0,0,2147483648,4294967295,0,0,0,4293918720,4294967295,8388607,0,0,3221225472,4294967295,0,0,0,4293918720,4294967295,8388607,0,0,3221225472,4294967295,0,0,0,4293918720,4294967295,8388607,0,0,3221225472,4294967295,0,0,0,4294443008,4294967295,8388607,0,0,3758096384,4294967295,0,0,0,4294443008,4294967295,4194303,0,0,3758096384,4294967295,0,0,0,4294705152,4294967295,4194303,0,0,3758096384,4294967295,0,0,0,4294705152,4294967295,4194303,0,0,3758096384,4294967295,0,0,0,4294705152,4294967295,4194303,0,0,4026531840,4294967295,0,0,0,4294836224,4294967295,2097151,0,0,4026531840,4294967295,0,0,0,4294836224,4294967295,2097151,0,0,4026531840,4294967295,0,0,0,4294836224,4294967295,2097151,0,0,4160749568,4294967295,0,0,0,4294901760,4294967295,1048575,0,0,4160749568,4294967295,0,0,0,4294901760,4294967295,1048575,0,0,4160749568,4294967295,0,0,0,4294934528,4294967295,1048575,0,0,4227858432,4294967295,0,0,0,4294934528,4294967295,524287,0,0,4227858432,4294967295,0,0,0,4294934528,4294967295,524287,0,0,4227858432,4294967295,0,0,0,4294950912,4294967295,262143,0,0,4227858432,4294967295,0,0,0,4294950912,4294967295,262143,0,0,4261412864,4294967295,0,0,0,4294959104,4294967295,131071,0,0,4261412864,4294967295,0,0,0,4294959104,4294967295,65535,0,0,4261412864,4294967295,0,0,0,4294963200,4294967295,65535,0,0,4278190080,4294967295,0,0,0,4294963200,4294967295,32767,0,0,4278190080,4294967295,0,0,0,4294965248,4294967295,16383,0,0,4278190080,4294967295,0,0,0,4294965248,4294967295,8191,0,0,4286578688,4294967295,0,0,0,4294966272,4294967295,4095,0,0,4286578688,4294967295,0,0,0,4294966272,4294967295,2047,0,0,4290772992,4294967295,0,0,0,4294966784,4294967295,1023,0,0,4290772992,4294967295,0,0,0,4294966784,4294967295,511,0,0,4290772992,4294967295,0,0,0,4294967040,4294967295,255,0,0,4292870144,4294967295,0,0,0,4294967168,4294967295,63,0,0,4292870144,4294967295,0,0,0,4294967168,4294967295,15,0,0,4293918720,4294967295,0,0,0,4294967232,4294967295,3,0,0,4294443008,4294967295,0,0,0,4294967232,2147483647,0,0,0,4294443008,4294967295,0,0,0,4294967264,536870911,0,0,0,4294705152,1048575,0,0,0,4294967264,134217727,0,0,0,4294705152,32767,0,0,0,4294967280,33554431,0,0,0,4294836224,2047,0,0,0,4294967280,8388607,0,0,0,4294901760,511,0,0,0,4294967288,2097151,0,0,0,4294934528,63,0,0,0,4294967288,1048575,0,0,0,4294950912,15,0,0,0,4294967292,524287,0,0,0,4294959104,7,0,0,0,4294967292,262143,0,0,0,4294959104,3,0,0,0,4294967292,65535,0,0,0,4294963200,0,0,0,0,4294967294,32767,0,0,0,2147481600,0,0,0,0,4294967294,32767,0,0,0,1073740800,0,0,0,0,4294967295,16383,0,0,0,1073741312,0,0,0,0,4294967295,8191,0,0,0,536870656,0,0,0,2147483648,4294967295,4095,0,0,0,268435328,0,0,0,2147483648,4294967295,2047,0,0,0,268435392,0,0,0,2147483648,4294967295,2047,0,0,0,134217696,0,0,0,3221225472,4294967295,1023,0,0,0,134217712,0,0,0,3221225472,4294967295,511,0,0,0,134217720,0,0,0,3758096384,4294967295,511,0,0,0,67108860,0,0,0,3758096384,4294967295,255,0,0,0,67108862,0,0,0,4026531840,4294967295,127,0,0,0,67108863,0,0,0,4026531840,4294967295,127,0,0,2147483648,33554431,0,0,0,4026531840,4294967295,63,0,0,3221225472,33554431,0,0,0,4160749568,4294967295,63,0,0,3758096384,33554431,0,0,0,4160749568,4294967295,31,0,0,3758096384,33554431,0,0,0,4227858432,4294967295,31,0,0,4026531840,16777215,0,0,0,4227858432,4294967295,31,0,0,4160749568,16777215,0,0,0,4261412864,4294967295,15,0,0,4160749568,16777215,0,0,0,4261412864,4294967295,15,0,0,4227858432,16777215,0,0,0,4261412864,4294967295,7,0,0,4261412864,16777215,0,0,0,4278190080,4294967295,7,0,0,4261412864,16777215,0,0,0,4278190080,4294967295,7,0,0,4261412864,8388607,0,0,0,4286578688,4294967295,3,0,0,4278190080,8388607,0,0,0,4286578688,4294967295,3,0,0,4278190080,8388607,0,0,0,4286578688,4294967295,3,0,0,4286578688,8388607,0,0,0,4290772992,4294967295,3,0,0,4286578688,4194303,0,0,0,4290772992,4294967295,1,0,0,4286578688,4194303,0,0,0,4290772992,4294967295,1,0,0,4286578688,4194303,0,1,0,4292870144,4294967295,1,0,0,4286578688,4194303,0,1,0,4292870144,4294967295,1,0,0,4286578688,2097151,0,3,0,4292870144,4294967295,1,0,0,4286578688,2097151,0,7,0,4293918720,4294967295,0,0,0,4286578688,2097151,0,7,0,4293918720,4294967295,0,0,0,4286578688,2097151,0,15,0,4293918720,4294967295,0,0,0,4286578688,1048575,0,15,0,4293918720,4294967295,0,0,0,4286578688,1048575,0,31,0,4294443008,4294967295,0,0,0,4278190080,524287,0,31,0,4294443008,4294967295,0,0,0,4278190080,524287,0,31,0,4294443008,2147483647,0,0,0,4261412864,262143,0,63,0,4294443008,2147483647,0,0,0,4261412864,131071,0,63,0,4294443008,2147483647,0,0,0,4227858432,131071,0,63,0,4294443008,2147483647,0,0,0,4160749568,65535,0,63,0,4294443008,2147483647,0,0,0,4026531840,16383,0,127,0,4294443008,2147483647,0,0,0,3221225472,8191,0,127,0,4294443008,2147483647,0,0,0,0,2046,0,127,0,4294705152,2147483647,0,0,0,0,0,0,127,0,4294705152,2147483647,0,0,0,0,0,0,127,0,4294705152,1073741823,0,0,0,0,0,0,127,0,4294705152,1073741823,0,0,0,0,0,0,127,0,4294443008,1073741823,0,0,0,0,0,0,127,0,4294443008,1073741823,0,0,0,0,0,0,127,0,4294443008,1073741823,0,0,0,0,0,0,127,0,4294443008,1073741823,0,0,0,0,0,0,127,0,4294443008,1073741823,0,0,0,0,0,0,127,0,4294443008,1073741823,0,0,0,0,0,0,63,0,4294443008,1073741823,0,0,0,0,0,0,63,0,4294443008,1073741823,0,0,0,0,0,0,63,0,4294443008,1073741823,0,0,0,0,0,0,63,0,4293918720,1073741823,0,0,0,0,0,0,63,0,4293918720,1073741823,0,0,0,0,0,0,31,0,4293918720,1073741823,0,0,0,0,0,0,31,0,4293918720,536870911,0,0,0,0,0,0,31,0,4293918720,536870911,0,0,0,0,0,0,15,0,4292870144,536870911,0,0,0,0,0,0,15,0,4292870144,536870911,0,0,0,0,0,0,15,0,4292870144,536870911,0,0,0,0,0,0,7,0,4292870144,536870911,0,0,0,0,0,0,7,0,4290772992,536870911,0,0,0,0,0,0,3,0,4290772992,536870911,0,0,0,0,0,0,3,0,4290772992,536870911,0,0,0,0,0,0,1,0,4286578688,536870911,0,0,0,0,0,0,1,0,4286578688,268435455,0,0,0,0,0,0,1,0,4286578688,268435455,0,0,0,0,0,0,0,0,4278190080,268435455,0,0,0,0,0,0,0,0,4278190080,268435455,0,0,0,0,0,0,0,0,4278190080,268435455,0,3758096384,63,0,0,0,0,0,4261412864,268435455,0,4261412864,511,0,0,0,0,0,4261412864,268435455,0,4290772992,4095,0,0,0,0,0,4261412864,134217727,0,4293918720,8191,0,0,0,0,0,4227858432,134217727,0,4294705152,16383,0,0,0,0,0,4227858432,134217727,0,4294836224,65535,0,0,0,0,0,4227858432,134217727,0,4294934528,65535,0,0,0,0,0,4160749568,67108863,0,4294950912,131071,0,0,0,0,0,4160749568,67108863,0,4294959104,262143,0,0,0,0,0,4160749568,67108863,0,4294963200,524287,0,0,0,0,0,4026531840,67108863,0,4294963200,524287,0,0,0,0,0,4026531840,33554431,0,4294965248,1048575,0,0,0,0,0,4026531840,33554431,0,4294966272,1048575,0,0,0,0,0,3758096384,33554431,0,4294966272,2097151,0,0,62914560,0,0,3758096384,16777215,0,4294966784,2097151,0,0,266338304,0,0,3758096384,16777215,0,4294966784,4194303,0,0,267386880,0,0,3221225472,16777215,0,4294967040,4194303,0,0,535822336,0,0,3221225472,8388607,0,4294967040,8388607,0,0,535822336,0,0,3221225472,8388607,0,4294967040,8388607,0,0,535822336,0,0,3221225472,8388607,0,4294967040,8388607,0,0,267386880,0,0,2147483648,4194303,0,4294967040,16777215,0,0,266338304,0,0,2147483648,4194303,0,4294967040,16777215,0,0,62914560,0,0,2147483648,2097151,0,4294967040,16777215,0,0,0,0,0,2147483648,2097151,0,4294967040,33554431,0,0,0,0,0,0,1048575,0,4294967040,33554431,0,0,0,0,0,0,1048575,0,4294967040,33554431,0,0,0,0,0,0,1048575,0,4294967040,67108863,0,0,0,0,0,0,524287,0,4294967040,67108863,0,0,0,0,0,0,524287,0,4294967040,67108863,0,0,0,0,0,0,262143,0,4294966784,134217727,0,0,0,0,0,0,262142,0,4294966784,134217727,0,0,0,0,0,0,262142,0,4294966784,134217727,0,0,0,0,0,0,131070,0,4294966272,268435455,0,0,0,0,0,0,131070,0,4294966272,268435455,0,0,0,0,0,0,131070,0,4294965248,536870911,0,0,0,0,0,0,65534,0,4294965248,536870911,0,0,0,0,0,0,65534,0,4294963200,536870911,0,0,0,0,0,0,65535,0,4294963200,1073741823,0,0,0,0,0,0,65535,0,4294959104,1073741823,0,0,0,0,0,0,32767,0,4294959104,2147483647,0,0,0,0,0,0,32767,0,4294950912,2147483647,0,0,0,0,0,0,32767,0,4294950912,4294967295,0,0,0,0,0,0,32767,0,4294934528,4294967295,0,0,0,0,0,0,32767,0,4294934528,4294967295,1,0,0,0,0,0,16383,0,4294901760,4294967295,1,0,0,0,0,2147483648,16383,0,4294901760,4294967295,3,0,0,0,0,2147483648,16383,0,4294836224,4294967295,3,0,0,0,0,2147483648,16383,0,4294836224,4294967295,7,0,0,0,0,2147483648,16383,0,4294705152,4294967295,15,0,0,0,0,2147483648,8191,0,4294705152,4294967295,15,0,0,0,0,2147483648,8191,0,4294443008,4294967295,31,0,0,0,0,2147483648,8191,0,4294443008,4294967295,31,0,0,0,0,2147483648,8191,0,4293918720,4294967295,63,0,0,0,0,2147483648,4095,0,4293918720,4294967295,127,0,0,0,0,2147483648,4095,0,4292870144,4294967295,127,0,0,0,0,0,2047,0,4292870144,4294967295,255,0,0,0,0,0,2047,0,4290772992,4294967295,511,0,0,0,0,0,1023,0,4290772992,4294967295,511,0,0,0,0,0,510,0,4286578688,4294967295,1023,0,0,0,0,0,252,0,4286578688,4294967295,2047,0,0,0,0,0,48,0,4278190080,4294967295,4095,0,0,0,0,0,0,0,4278190080,4294967295,4095,0,0,0,0,0,0,0,4261412864,4294967295,8191,0,0,0,0,0,0,0,4261412864,4294967295,16383,0,0,0,0,0,0,0,4227858432,4294967295,16383,0,0,0,0,0,0,0,4227858432,4294967295,32767,0,0,0,0,0,0,0,4160749568,4294967295,65535,0,0,0,0,0,0,0,4160749568,4294967295,65535,0,0,0,0,0,0,0,4160749568,4294967295,131071,0,0,0,0,0,0,0,4026531840,4294967295,262143,0,0,0,0,0,0,0,4026531840,4294967295,262143,0,0,0,0,0,0,0,3758096384,4294967295,524287,0,0,0,0,0,0,0,3758096384,4294967295,524287,0,0,0,0,0,0,0,3221225472,4294967295,1048575,0,0,0,0,0,0,0,3221225472,4294967295,1048575,0,0,0,0,0,0,0,3221225472,4294967295,2097151,0,0,0,0,0,0,0,2147483648,4294967295,2097151,0,0,0,0,0,0,0,2147483648,4294967295,2097151,0,0,0,0,0,0,0,2147483648,4294967295,4194303,0,0,0,0,0,0,0,0,4294967295,4194303,0,0,0,0,0,0,0,0,4294967295,4194303,0,0,0,0,0,0,0,0,4294967295,4194303,0,0,0,0,0,0,0,0,4294967294,8388607,0,0,0,0,0,0,0,0,4294967294,8388607,0,0,0,0,0,0,0,0,4294967294,8388607,0,0,0,0,0,0,0,0,4294967294,8388607,0,0,0,0,0,0,0,0,4294967292,8388607,0,0,0,0,0,0,0,0,4294967292,8388607,0,0,0,0,0,0,0,0,4294967292,8388607,0,0,0,0,0,0,0,0,4294967292,8388607,0,0,0,0,0,0,0,0,4294967288,8388607,0,0,0,0,0,0,0,0,4294967288,8388607,0,0,0,0,0,0,0,0,4294967288,8388607,0,0,0,0,0,0,0,0,4294967288,8388607,0,0,0,0,0,0,0,0,4294967288,4194303,0,0,0,0,0,0,0,0,4294967280,4194303,0,0,0,0,0,0,0,0,4294967280,4194303,0,0,0,0,0,0,0,0,4294967280,4194303,0,0,0,0,0,0,0,0,4294967280,4194303,0,0,0,0,0,0,0,0,4294967280,2097151,0,0,0,0,0,0,0,0,4294967280,2097151,0,0,0,0,0,0,0,0,4294967280,2097151,0,0,0,0,0,0,0,0,4294967280,1048575,0,0,0,0,0,0,0,0,4294967280,1048575,0,0,0,0,0,0,0,0,4294967280,1048575,0,0,0,0,0,0,0,0,4294967264,524287,0,0,0,0,0,0,0,0,4294967264,524287,0,0,0,0,0,0,0,0,4294967264,524287,0,0,0,0,0,0,0,0,4294967264,262143,0,0,0,0,0,0,0,0,4294967264,262143,0,0,0,0,0,0,0,0,4294967264,131071,0,0,0,0,0,0,0,0,4294967264,131071,0,0,0,0,0,0,0,0,4294967264,65535,0,0,0,0,0,0,0,0,4294967280,65535,0,0,0,0,0,0,0,0,4294967280,65535,0,0,0,0,0,0,0,0,4294967280,32767,0,0,0,0,0,0,0,0,4294967280,32767,0,0,0,0,0,0,0,0,4294967280,16383,0,0,0,0,0,0,0,0,4294967280,16383,0,0,0,0,0,0,0,0,4294967280,8191,0,0,0,0,0,0,0,0,4294967280,8191,0,0,0,0,0,0,0,0,4294967280,4095,0,0,0,0,0,0,0,0,4294967288,4095,0,0,0,0,0,0,0,0,4294967288,4095,0,0,0,0,0,0,0,0,4294967288,2047,0,0,0,0,0,0,0,0,4294967288,2047,0,0,0,0,0,0,0,0,4294967288,1023,0,0,0,0,0,0,0,0,4294967292,1023,0,0,0,0,0,0,0,0,4294967292,1023,0,0,0,0,0,0,0,0,4294967292,511,0,0,0,0,0,0,0,0,4294967294,511,0,0,0,0,0,0,0,0,4294967294,255,0,0,0,0,0,0,0,0,4294967294,255,0,0,0,0,0,0,0,0,4294967295,255,0,0,0,0,0,0,0,0,4294967295,255,0,0,0,0,0,0,0,0,4294967295,127,0,0,0,0,0,0,0,2147483648,4294967295,127,0,0,0,0,0,0,0,2147483648,4294967295,127,0,0,0,0,0,0,0,3221225472,4294967295,63,0,0,1056964608,0,0,0,0,3221225472,4294967295,63,0,0,4290772992,1,0,0,0,3758096384,4294967295,63,0,0,4292870144,3,0,0,0,3758096384,4294967295,63,0,0,4293918720,15,0,0,0,4026531840,4294967295,63,0,0,4294443008,31,0,0,0,4026531840,4294967295,63,0,0,4294443008,63,0,0,0,4160749568,4294967295,31,0,0,4294705152,127,0,0,0,4160749568,4294967295,31,0,0,4294705152,255,0,0,0,4227858432,4294967295,31,0,0,4294836224,511,0,0,0,4227858432,4294967295,31,0,0,4294836224,1023,0,0,0,4261412864,4294967295,31,0,0,4294901760,2047,0,0,0,4278190080,4294967295,31,0,0,4294901760,2047,0,0,0,4278190080,4294967295,31,0,0,4294934528,4095,0,0,0,4286578688,4294967295,31,0,0,4294934528,8191,0,0,0,4286578688,4294967295,31,0,0,4294934528,16383,0,0,0,4290772992,4294967295,31,0,0,4294950912,32767,0,0,0,4292870144,4294967295,31,0,0,4294950912,32767,0,0,0,4292870144,4294967295,31,0,0],
  [4294963200,4294967295,15,0,0,0,0,3221225472,4294967295,4095,4294963200,4294967295,15,0,0,0,0,3221225472,4294967295,2047,4294963200,4294967295,31,0,0,0,0,2147483648,4294967295,1023,4294965248,4294967295,31,0,0,0,0,2147483648,4294967295,1023,4294965248,4294967295,31,0,0,0,0,2147483648,4294967295,511,4294965248,4294967295,63,0,0,0,0,0,4294967295,255,4294965248,4294967295,63,0,0,0,0,0,4294967295,255,4294965248,4294967295,63,0,0,0,0,0,4294967294,127,4294965248,4294967295,127,0,0,0,0,0,4294967294,63,4294965248,4294967295,127,0,0,0,0,0,4294967292,63,4294965248,4294967295,127,0,0,0,0,0,4294967292,31,4294965248,4294967295,127,0,0,0,0,0,4294967288,15,4294965248,4294967295,255,0,0,0,0,0,4294967288,15,4294965248,4294967295,255,0,0,0,0,0,4294967280,7,4294965248,4294967295,255,0,0,0,0,0,4294967280,3,4294965248,4294967295,255,0,0,0,0,0,4294967264,3,4294965248,4294967295,255,0,0,0,0,0,4294967264,1,4294965248,4294967295,255,0,0,0,0,0,4294967232,0,4294963200,4294967295,255,0,0,0,0,0,4294967168,0,4294963200,4294967295,255,0,0,0,0,0,2147483520,0,4294963200,4294967295,255,0,0,0,0,0,1073741568,0,4294963200,4294967295,255,0,0,0,0,0,1073741312,0,4294963200,4294967295,255,0,0,0,0,0,536869888,0,4294963200,4294967295,255,0,0,0,0,0,268433408,0,4294963200,4294967295,255,0,0,0,0,0,134213632,0,4294963200,4294967295,255,0,0,0,0,0,67100672,0,4294963200,4294967295,255,0,0,0,0,0,16744448,0,4294963200,4294967295,127,0,0,0,0,0,3932160,0,4294963200,4294967295,127,0,0,0,0,0,0,0,4294963200,4294967295,127,0,0,0,0,0,0,0,4294963200,4294967295,63,0,0,0,0,0,0,0,4294963200,4294967295,63,0,0,0,0,0,0,0,4294959104,4294967295,31,0,0,0,0,0,0,0,4294959104,4294967295,31,0,0,0,0,0,0,0,4294959104,4294967295,15,0,0,0,0,0,0,0,4294959104,4294967295,7,0,0,0,0,0,0,0,4294959104,4294967295,7,0,0,0,0,0,0,0,4294950912,4294967295,3,0,0,0,0,0,0,0,4294950912,4294967295,1,0,0,0,0,0,0,0,4294950912,4294967295,0,0,0,0,0,0,0,0,4294934528,2147483647,0,0,0,0,0,0,0,0,4294934528,1073741823,0,0,0,0,0,0,0,0,4294901760,536870911,0,0,0,0,0,0,0,0,4294901760,134217727,0,0,0,0,0,0,0,0,4294836224,67108863,0,0,0,0,0,0,0,0,4294705152,16777215,0,0,0,0,0,0,0,0,4294443008,4194303,0,0,0,0,0,0,0,0,4292870144,524287,0,0,0,0,0,0,0,0,4286578688,65535,0,0,0,0,0,0,0,0,4227858432,2047,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,16380,0,0,0,0,0,0,0,0,0,65535,0,0,0,0,0,0,0,0,3221225472,262143,0,0,0,0,0,0,0,0,4026531840,1048575,0,0,0,0,0,0,0,0,4160749568,2097151,0,0,0,0,0,0,0,0,4227858432,4194303,0,0,0,0,0,0,0,0,4261412864,8388607,0,0,0,0,917504,0,0,0,4278190080,16777215,0,0,0,0,4161536,0,0,0,4286578688,33554431,0,0,0,0,16760832,0,0,0,4290772992,67108863,0,0,0,0,33546240,0,0,0,4292870144,67108863,0,0,0,0,67100672,0,0,0,4292870144,134217727,0,0,0,0,67100672,0,0,0,4293918720,134217727,0,8372224,0,0,134213632,0,0,0,4294443008,268435455,0,67106816,0,0,134213632,0,0,0,4294443008,268435455,0,268435200,0,0,268431360,0,0,0,4294705152,268435455,0,536870784,0,0,268431360,0,0,0,4294705152,536870911,0,1073741792,0,0,268431360,0,0,0,4294836224,536870911,0,2147483632,0,0,268431360,0,0,0,4294836224,536870911,0,4294967280,0,0,268431360,0,0,0,4294901760,268435455,0,4294967288,1,0,268427264,0,0,0,4294901760,268435455,0,4294967292,1,0,268427264,0,0,0,4294934528,268435455,0,4294967292,3,0,134209536,0,0,0,4294934528,268435455,0,4294967292,3,0,134201344,0,0,0,4294950912,134217727,0,4294967292,7,0,134201344,0,0,0,4294950912,134217727,0,4294967294,7,0,67076096,0,0,0,4294959104,67108863,0,4294967294,7,0,33488896,0,0,0,4294959104,67108863,0,4294967292,7,0,16646144,0,0,0,4294959104,33554431,0,4294967292,15,0,1048576,0,0,0,4294963200,16777215,0,4294967292,15,0,0,0,0,0,4294963200,16777215,0,4294967292,15,0,0,0,0,0,4294965248,8388607,0,4294967292,15,0,0,0,0,0,4294965248,4194303,0,4294967288,15,0,0,0,0,0,4294966272,2097151,0,4294967288,15,0,0,0,0,0,4294966272,1048575,0,4294967280,15,0,0,0,0,0,4294966784,524287,0,4294967280,15,0,0,0,0,0,4294966784,262143,0,4294967264,15,0,0,0,0,0,4294967040,262143,0,4294967264,15,0,0,0,0,0,4294967040,131071,0,4294967232,15,0,0,0,0,0,4294967168,65535,0,4294967232,15,0,0,0,0,0,4294967232,32767,0,4294967168,15,0,0,0,0,0,4294967232,16383,0,4294967040,7,0,0,0,0,0,4294967264,8191,0,4294967040,7,0,0,0,0,0,4294967264,4095,0,4294966784,7,0,0,0,0,0,4294967280,2047,0,4294966784,7,0,0,0,0,0,4294967288,2047,0,4294966272,7,0,0,0,0,0,4294967292,1023,0,4294965248,3,0,0,0,0,0,4294967292,511,0,4294965248,3,0,0,0,0,0,4294967294,511,0,4294963200,3,0,0,0,0,0,4294967295,255,0,4294963200,3,0,0,0,0,2147483648,4294967295,127,0,4294959104,1,0,0,0,0,3221225472,4294967295,127,0,4294950912,1,0,0,0,0,3221225472,4294967295,63,0,4294950912,1,0,0,0,0,3758096384,4294967295,63,0,4294934528,1,0,0,0,0,4026531840,4294967295,31,0,4294934528,0,0,0,0,0,4160749568,4294967295,31,0,4294901760,0,0,0,0,0,4160749568,4294967295,15,0,2147352576,0,0,0,0,0,4227858432,4294967295,15,0,2147221504,0,0,0,0,0,4261412864,4294967295,7,0,1073479680,0,0,0,0,0,4261412864,4294967295,7,0,1073217536,0,0,0,0,0,4278190080,4294967295,3,0,535822336,0,0,0,0,0,4278190080,4294967295,3,0,266338304,0,0,0,0,0,4286578688,4294967295,3,0,50331648,0,0,0,0,0,4286578688,4294967295,1,0,0,0,0,0,0,0,4290772992,4294967295,1,0,0,0,0,0,0,0,4290772992,4294967295,0,0,0,0,0,0,0,0,4290772992,4294967295,0,0,0,0,0,0,0,0,4292870144,2147483647,0,0,0,0,0,0,0,0,4292870144,2147483647,0,0,0,0,0,0,0,0,4292870144,1073741823,0,0,0,0,0,0,0,0,4292870144,1073741823,0,0,0,0,0,0,0,0,4292870144,536870911,0,0,0,0,0,0,0,0,4292870144,536870911,0,0,0,0,0,0,0,0,4292870144,268435455,0,0,0,0,0,0,0,0,4290772992,134217727,0,0,0,0,0,0,0,0,4290772992,134217727,0,0,0,0,0,0,0,0,4290772992,67108863,0,0,0,0,0,0,0,0,4286578688,33554431,0,0,0,0,0,0,0,0,4286578688,16777215,0,0,0,0,0,0,0,0,4278190080,8388607,0,0,0,0,0,0,0,0,4261412864,2097151,0,0,0,0,0,0,0,0,4227858432,1048575,0,0,0,0,0,0,0,0,4026531840,262143,0,0,0,0,0,0,0,0,3221225472,65535,0,0,0,0,0,0,0,0,0,4094,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1073479680,0,0,0,0,0,0,0,0,0,4294950912,3,0,0,0,0,0,0,0,0,4294963200,15,0,0,0,0,0,0,0,0,4294966272,63,0,0,0,0,0,0,0,0,4294967040,127,0,0,0,0,0,0,0,0,4294967168,255,0,0,0,0,0,0,0,0,4294967232,511,0,0,0,0,0,0,0,0,4294967264,1023,0,0,0,0,0,0,0,0,4294967280,1023,0,0,0,0,0,126976,0,0,4294967288,2047,0,0,0,0,0,260096,0,0,4294967288,2047,0,0,0,0,0,523264,0,0,4294967292,2047,0,0,0,0,0,1048064,0,0,4294967294,2047,0,0,0,0,0,1048064,0,0,4294967294,2047,0,0,0,0,0,2096896,0,0,4294967295,2047,0,0,0,0,0,2096896,0,0,4294967295,2047,0,0,0,0,0,2096896,0,0,4294967295,2047,0,0,0,0,0,2096896,0,2147483648,4294967295,1023,0,0,0,0,0,2096896,0,2147483648,4294967295,1023,0,0,0,0,0,2096896,0,2147483648,4294967295,1023,0,0,0,0,0,2096896,0,2147483648,4294967295,511,0,0,0,0,0,2096640,0,3221225472,4294967295,255,0,0,0,0,0,1048064,0,3221225472,4294967295,255,0,0,0,0,0,1047552,0,3221225472,4294967295,127,0,0,0,0,0,522240,0,3221225472,4294967295,127,0,0,0,0,0,258048,0,3221225472,4294967295,63,0,0,0,0,0,49152,0,3221225472,4294967295,31,0,0,0,0,0,0,0,3221225472,4294967295,31,0,0,0,0,0,0,0,3758096384,4294967295,15,0,0,0,0,0,0,0,3758096384,4294967295,15,0,0,0,0,0,0,0,3758096384,4294967295,7,0,0,0,0,0,0,0,3758096384,4294967295,3,0,0,0,0,0,0,0,3758096384,4294967295,3,0,0,0,0,0,0,0,3758096384,4294967295,1,0,0,0,0,0,0,0,3758096384,4294967295,0,0,0,0,0,0,0,0,3758096384,4294967295,0,0,0,0,0,0,0,0,3758096384,2147483647,0,0,0,0,0,0,0,0,3758096384,1073741823,0,0,0,0,0,0,0,0,3758096384,1073741823,0,0,0,0,0,0,0,0,3221225472,536870911,0,0,0,0,0,0,0,0,3221225472,268435455,0,0,0,0,0,0,0,0,3221225472,268435455,0,0,0,0,0,0,0,0,3221225472,134217727,0,0,0,0,0,0,0,0,3221225472,134217727,0,0,0,0,0,0,0,0,3221225472,67108863,0,0,62914560,0,0,0,0,0,3221225472,33554431,0,0,1073479680,0,0,0,0,0,3221225472,33554431,0,0,4294934528,0,0,0,0,0,3221225472,16777215,0,0,4294959104,3,0,0,0,0,3221225472,16777215,0,0,4294963200,7,0,0,0,0,2147483648,8388607,0,0,4294966272,15,0,0,0,0,2147483648,4194303,0,0,4294966784,31,0,0,0,0,2147483648,2097151,0,0,4294967168,63,0,0,0,0,2147483648,2097151,0,0,4294967232,127,0,0,0,0,0,1048575,0,0,4294967280,255,0,0,0,0,0,524287,0,0,4294967288,511,0,0,0,0,0,262142,0,0,4294967294,1023,0,0,0,0,0,131068,0,1,4294967295,1023,0,0,0,0,0,65532,0,3221225487,4294967295,2047,0,0,0,0,0,32760,0,4026531903,4294967295,4095,0,0,0,0,0,8160,0,4227858943,4294967295,4095,0,0,0,0,0,896,0,4286586879,4294967295,8191,0,0,0,0,0,0,0,4294967295,4294967295,16383,0,0,0,0,0,0,0,4294967295,4294967295,16383,0,0,0,0,0,0,0,4294967295,4294967295,32767,0,0,0,0,0,0,0,4294967295,4294967295,32767,0,0,0,0,0,0,0,4294967295,4294967295,65535,0,0,0,0,0,0,0,4294967295,4294967295,65535,0,0,0,0,0,0,0,4294967295,4294967295,65535,0,0,0,0,0,0,0,4294967295,4294967295,131071,0,0,0,0,0,0,0,4294967295,4294967295,131071,0,0,0,0,0,0,0,4294967295,4294967295,262143,0,0,0,0,0,0,0,4294967295,4294967295,262143,0,0,0,0,0,0,0,4294967295,4294967295,262143,0,0,0,0,0,0,0,4294967295,4294967295,262143,0,0,0,0,0,0,0,4294967295,4294967295,524287,0,0,0,0,0,0,0,4294967295,4294967295,524287,0,0,0,0,0,0,0,4294967295,4294967295,524287,0,0,0,0,0,0,0,4294967295,4294967295,524287,0,0,0,0,0,0,0,4294967295,4294967295,524287,0,0,0,0,0,0,0,4294967295,4294967295,524287,0,0,0,0,0,0,0,4294967295,4294967295,524287,0,0,0,0,0,0,0,4294967295,4294967295,262143,0,0,0,0,0,0,0,4294967295,4294967295,262143,0,0,0,0,0,0,0,4294967295,4294967295,131071,0,0,0,0,0,0,0,4294967295,4294967295,65535,0,0,0,0,0,0,0,4294967295,4294967295,8191,0,0,0,0,0,0,0,4294967295,16383,0,0,0,0,0,0,0,0,4294967295,511,0,0,0,0,0,0,0,0,4294967295,127,0,0,0,0,0,0,0,0,4294967295,31,0,0,0,0,0,0,0,0,4294967295,7,0,0,0,0,0,0,0,0,4294967295,3,0,0,0,0,0,0,0,0,4294967295,0,0,0,0,0,0,0,0,0,2147483647,0,0,0,0,0,0,0,0,0,1073741823,0,0,0,0,0,0,0,0,0,536870911,0,0,0,0,0,0,0,0,0,268435455,0,0,0,0,0,0,0,0,0,134217727,0,0,0,0,0,0,0,0,0,134217727,0,0,0,0,0,0,0,0,0,67108863,0,0,0,0,0,0,0,0,0,33554431,0,0,0,0,0,0,0,0,0,16777215,0,0,0,0,0,0,0,0,0,8388607,0,0,0,0,0,0,0,0,0,8388607,0,0,0,0,0,0,0,0,0,4194303,0,0,0,0,0,0,0,0,0,2097151,0,0,0,0,0,0,0,0,0,2097151,0,0,0,0,0,0,0,0,0,1048575,0,0,0,0,0,0,0,0,0,524287,0,0,0,0,0,0,0,0,0,262143,0,0,0,0,0,0,0,0,0,262143,0,0,0,0,0,0,0,0,0,131071,0,0,0,0,0,0,0,0,0,65535,0,0,0,0,0,0,0,0,0,32767,0,0,0,0,0,0,0,0,0,32767,0,0,0,0,0,0,0,0,0,16383,0,0,0,0,0,0,0,0,0,8191,0,0,0,0,0,0,0,0,0,4095,0,0,0,0,0,0,0,0,0,4095,0,0,0,0,0,0,0,0,0,2047,0,0,0,0,0,0,0,0,0,1023,0,0,0,0,0,0,0,0,0,511,0,0,0,0,0,0,0,0,0,255,0,0,0,0,0,0,0,0,0,255,0,0,0,0,0,0,0,0,0,127,0,0,0,0,0,0,0,0,0,63,0,0,0,0,0,0,0,0,0,31,0,0,0,0,0,0,0,0,0,15,0,0,0,0,0,0,0,0,0,7,0,0,0,0,0,0,0,0,0,3,0,0,0,0,0,0,0,0,0,1,0,0,0,0,62914560,0,0,0,0,0,0,0,0,0,267386880,0,0,0,0,0,0,0,0,0,1073217536,0,0,0,0,0,0,0,0,0,2147221504,0,0,0,0,0,0,0,0,0,4294836224,0,0,0,0,0,0,0,0,0,4294836224,1,0,0,0,0,0,0,0,0,4294901760,3,0,0,0,0,0,0,0,0,4294901760,3,0,0,0,0,0,0,0,0,4294934528,7,0,0,0,0,0,0,0,0,4294934528,15,0,0,0,0,0,0,0,0,4294934528,15,0,0,0,0,0,0,0,0,4294934528,31,0,0,0,0,0,0,0,0,4294950912,63,0,0,0,0,0,0,0,0,4294950912,63,0,0,0,0,0,0,0,0,4294950912,127,0,0,0,0,0,0,0,0,4294950912,127,0,0,0,0,0,0,0,0,4294950912,255,0,0,0,0,0,0,0,0,4294950912,255,0,0,0,0,0,0,0,0,4294950912,511,0,0,0,0,0,0,0,0,4294950912,1023,0,0,0,0,0,0,0,0,4294950912,1023,0,0,0,0,0,0,0,0,4294950912,2047,0,0,0,0,0,0,0,0,4294950912,2047,0,0,0,0,0,0,0,0,4294950912,4095,0,0,0,0,0,0,0,0,4294950912,4095,0,1073676288,0,0,0,0,0,0,4294950912,8191,0,4294959104,1,0,0,0,0,0,4294950912,16383,0,4294966272,15,0,0,0,0,0,4294950912,16383,0,4294967040,63,0,0,0,14,0,4294959104,32767,0,4294967232,255,0,0,0,63,0,4294959104,65535,0,4294967280,1023,0,0,2147483648,127,0,4294959104,65535,0,4294967288,4095,0,0,3221225472,127,0,4294959104,131071,0,4294967294,16383,0,0,3221225472,255,0,4294959104,262143,0,4294967295,65535,0,0,3758096384,255,0,4294959104,262143,3221225472,4294967295,262143,0,0,3758096384,511,0,4294959104,524287,3758096384,4294967295,2097151,0,0,4026531840,511,0,4294959104,1048575,4026531840,4294967295,8388607,0,0,4026531840,511,0,4294959104,2097151,4160749568,4294967295,67108863,0,0,4026531840,1023,0,4294959104,2097151,4261412864,4294967295,536870911,0,0,4160749568,1023,0,4294959104,4194303,4278190080,4294967295,4294967295,0,0,4160749568,1023,0,4294963200,8388607,4286578688,4294967295,4294967295,0,0,4160749568,1023,0,4294963200,16777215,4290772992,4294967295,4294967295,0,0,4160749568,1023,0,4294963200,33554431,4292870144,4294967295,4294967295,0,0,4227858432,2047,0,4294963200,33554431,4293918720,4294967295,4294967295,0,0,4227858432,2047,0,4294963200,67108863,4294443008,4294967295,4294967295,0,0,4227858432,2047,0,4294963200,134217727,4294705152,4294967295,4294967295,0,0,4227858432,2047,0,4294965248,268435455,4294836224,4294967295,4294967295,0,0,4227858432,2047,0,4294965248,536870911,4294901760,4294967295,4294967295,0,0,4227858432,2047,0,4294965248,536870911,4294901760,4294967295,4294967295,0,0,4227858432,2047,0,4294965248,1073741823,4294934528,4294967295,4294967295,0,0,4227858432,2047,0,4294965248,2147483647,4294950912,4294967295,4294967295,0,0,4227858432,2047,0,4294966272,4294967295,4294950912,4294967295,4294967295,0,0,4227858432,2047,0,4294966272,4294967295,4294950912,4294967295,4294967295,0,0,4227858432,2047,0,4294966272,4294967295,4294959105,4294967295,4294967295,0,0,4227858432,2047,0,4294966272,4294967295,4294959107,4294967295,4294967295,0,0,4227858432,1023,0,4294966784,4294967295,4294959107,4294967295,4294967295,0,0,4160749568,1023,0,4294966784,4294967295,4294959107,4294967295,4294967295,0,0,4160749568,1023,0,4294966784,4294967295,4294950919,4294967295,4294967295,0,0,4026531840,511,0,4294967040,4294967295,4294934535,4294967295,4294967295,0,0,4026531840,255,0,4294967040,4294967295,4294901763,4294967295,4294967295,0,0,3758096384,127,0,4294967040,4294967295,4294836227,4294967295,4294967295,0,0,3221225472,63,0,4294967168,4294967295,4294443011,4294967295,4294967295,0,0,0,14,0,4294967168,4294967295,4290772993,4294967295,4294967295,0,0,0,0,0,4294967168,4294967295,4227858433,4294967295,4294967295,0,0,0,0,0,4294967232,4294967295,3758096384,4294967295,4294967295,0,0,0,0,0,4294967232,4294967295,0,4294967295,4294967295,0,0,0,0,0,4294967264,2147483647,0,4294967280,4294967295,0,0,0,0,0,4294967264,2147483647,0,4294967168,4294967295,0,0,0,0,0,4294967280,1073741823,0,4294966784,4294967295,0,0,0,0,0,4294967280,536870911,0,4294963200,4294967295,0,0,0,0,0,4294967288,536870911,0,4294934528,4294967295,0,0,0,0,0,4294967292,268435455,0,4294836224,4294967295,0,0,0,0,0,4294967294,134217727,0,4294443008,4294967295,0,0,0,0,0,4294967294,134217727,0,4292870144,4294967295,0,0,0,0,0,4294967295,67108863,0,4286578688,4294967295,0,0,0,0,3221225472,4294967295,33554431,0,4261412864,4294967295,0,0,0,0,3758096384,4294967295,16777215,0,4227858432,4294967295,0,0,0,0,4160749568,4294967295,8388607,0,4026531840,4294967295,0,0,0,0,4261412864,4294967295,4194303,0,3758096384,4294967295,0,0,0,0,4290772992,4294967295,2097151,0,2147483648,4294967295,0,0,0,0,4294443008,4294967295,1048575,0,0,4294967295,0,0,0,0,4294901760,4294967295,524287,0,0,4294967294,0,0,0,0,4294950912,4294967295,262143,0,0,4294967292,0,0,0,0,4294963200,4294967295,65535,0,0,4294967280,0,0,0,0,4294965248,4294967295,32767,0,0,4294967264,0,0,0,0,4294966784,4294967295,8191,0,0,4294967232,0,4193792,0,0,4294967040,4294967295,2047,0,0,4294967168,0,67108856,0,0,4294967168,4294967295,255,0,0,4294967040,0,268435455,0,0,4294967168,4294967295,15,0,0,4294966784,3758096384,536870911,0,0,4294967232,1073741823,0,0,0,4294966272,4160749568,2147483647,0,0,4294967232,4194303,0,0,0,4294965248,4261412864,2147483647,0,0,4294967264,131071,0,0,0,4294963200,4286578688,4294967295,0,0,4294967264,16383,0,0,0,4294959104,4290772992,4294967295,1,0,4294967264,4095,0,0,0,4294950912,4293918720,4294967295,1,0,4294967280,1023,0,0,0,4294950912,4294443008,4294967295,3,0,4294967280,511,0,0,0,4294934528,4294705152,4294967295,3,0,4294967280,255,0,0,0,4294901760,4294836224,4294967295,7,0,4294967280,127,0,0,0,4294836224,4294901760,4294967295,7,0,4294967280,63,0,0,0,4294705152,4294901760,4294967295,7,0,4294967280,63,0,0,0,4294443008,4294934528,4294967295,7,0,4294967280,31,0,0,0,4293918720,4294950912,4294967295,15,0,4294967280,15,0,0,0,4292870144,4294950912,4294967295,15,0,4294967280,15,0,0,0,4290772992,4294959104,4294967295,15,0,4294967280,7,0,0,0,4286578688,4294959104,4294967295,15,0,4294967280,3,0,0,0,4278190080,4294963200,4294967295,15,0,4294967264,3,0,0,0,4261412864,4294963200,4294967295,15,0,4294967264,1,0,0,0,4227858432,4294965248,4294967295,15,0,4294967264,1,0,0,0,4227858432,4294965248,4294967295,31,0,4294967264,0,0,0,0,4160749568,4294965248,4294967295,31,0,4294967264,0,0,0,0,4026531840,4294966272,4294967295,31,0,2147483584,0,0,0,0,3758096384,4294966272,4294967295,31,0,1073741760,0,0,0,0,3221225472,4294966272,4294967295,31,0,1073741760,0,0,0,0,2147483648,4294966272,4294967295,31,0,536870784,0,0,0,0,0,4294966272,4294967295,31,0,536870784,0,0,0,0,0,4294966272,4294967295,31,0,268435328,0,0,0,0,0,4294966272,4294967295,31,0,134217472,0,0,0,0,0,4294966272,4294967295,31,0,134217472,0,0,0,0,0,4294966272,4294967295,31,0,67108352,0,0,0,0,0,4294966272,4294967295,31,0,33553920,0,0,0,0,0,4294966272,4294967295,31,0,16776192,0,0,0,0,0,4294966272,4294967295,31,0,16775168,0,0,0,0,0,4294966272,4294967295,31,0,4190208,0,0,0,0,0,4294966272,4294967295,31,0,2088960,0,0,0,0,0,4294966272,4294967295,31,0,491520,0,0,0,0,0,4294966272,4294967295,31,0,0,0,0,0,0,0,4294966272,4294967295,31,0,0,0,0,0,0,0,4294966272,4294967295,31,0,0,0,0,0,0,0,4294965248,4294967295,31,0,0,0,0,0,0,0,4294965248,4294967295,31,0,0,0,0,0,0,0,4294965248,4294967295,31,0,0,0,0,0,0,0,4294965248,4294967295,31,0,0,0,0,0,0,0,4294965248,4294967295,31,0,0,0,0,0,0,0,4294963200,4294967295,63,0,0,0,0,0,0,0,4294963200,4294967295,63,0,0,0,0,0,0,0],
];
