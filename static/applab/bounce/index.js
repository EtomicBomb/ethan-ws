var TAU = 2*Math.PI;
var CANNON_LOOP_MS = 20;
var PHYSICS_LOOP_MS = 20;
var SHOT_V0 = 12000/1000;
var SHOT_RADIUS = 10;
var GRAVITY_STRENGTH = 0.25;
var BARRIER_DISTANCE = 50;


var myCannon = new Cannon();

var score;
var moves;
var circles;


startSingleplayer();
onEvent("playAgain", "click", restartSingleplayer);

function startSingleplayer() {
  
  onEvent("main", "mousedown", pressHandler);
  onEvent("main", "keydown", pressHandler);
  
  score = 0;
  moves = 0;
  circles = [];
  
  setScreen("main");
  
  createShot();
  createBackground();
  
  myCannon.start();
}

function restartSingleplayer() {
  for (var i=0; i<circles.length; i++) {
    circles[i].delete();
  }
  setText("scoreLabel", "Score: 0");
  hideElement("shot"); 


  circles = [];
  
  score = 0;
  moves = 0;
  
  setScreen("main");
  
  myCannon.start();
}




function Cannon() {
  this.isActive = false;
  this.dAngle = 3*20/1000;
  this.angle = 0;

  this.start = function () {
    if (this.isActive) return;
    this.isActive = true;
    this.loop();
  };
  
  this.stop = function () {
    this.isActive = false;
  };
  
  this.loop = function () {
     if (!this.isActive) return;
    
    var newCannonAngle = this.angle + this.dAngle;
    if (this.angle > TAU/2 || newCannonAngle < 0) this.dAngle = -this.dAngle;

    this.angle += this.dAngle;
    setStyle("cannon", "transform: rotate("+ (-this.angle) +"rad);");

    setTimeout(function() {
      that.loop();
    }, CANNON_LOOP_MS);
  };
  
  var that = this;
}


function pressHandler() {
  // keypress or mouse
  if (!myCannon.isActive) return;
  
  var cannonAngle = myCannon.angle;
  myCannon.stop();
  
  
  showElement("shot");
  var shot = new Shot(160, 450-SHOT_RADIUS, SHOT_V0, cannonAngle);
  
  moves++;
  
  var physicsLoop = timedLoop(PHYSICS_LOOP_MS, function() {
    if (shot.v < 0) {
      hideElement("shot");
      var newCircle = circleFromLargestPossible(shot.x, shot.y);
      circles.push(newCircle);
      
      myCannon.start();
      stopTimedLoop(physicsLoop);
      return;
    }

    if (shot.y > 450-BARRIER_DISTANCE && normalize(shot.angle) > TAU/2) {
      // they lost 
      createRecord("singleplayerScores", {score:score,moves:moves}, function() {
        highScores(score);
      });
      
      stopTimedLoop(physicsLoop);
      return;  
    }
    
    shot.step();
  });
}
 
function highScores(score) {
  readRecords("singleplayerScores", {}, function(scores) {
    setText("yourScore", "Your score: "+score);
    
    scores.sort(function(a, b) {
      if (a.score > b.score) {
        return -1;
      } else {
        return 1;
      }
    });
    
    var text = "";
    for (var i=0; i<Math.min(10, scores.length); i++) {
      text += (i+1) + ": " + scores[i].score + " in " + scores[i].moves + " moves\n";
    }
    
    setText("highScoresLabel", text);
    
    setScreen("highScores");
  });
}

function circleFromLargestPossible(x, y) {
  var distances = [
    x, // left wall
    320 - x, // right wall
    
    y, // top wall
    450-BARRIER_DISTANCE-y, // bottom wall
  ];
  
  for (var i=0; i<circles.length; i++) {
    var circle = circles[i];
    var circleDistance = distance(x, y, circle.x, circle.y) - circle.r;
    distances.push(circleDistance);
  }
  
  var radius = Math.min.apply(Math, distances);

  return new Circle(x, y, radius);
}

function Circle(x, y, r) {
  this.hitsLeft = 3;
  this.x = x;
  this.y = y;
  this.r = r;
  this.id = uniqueId();
  
  image(this.id, "");
  setSize(this.id, 0, 0);

  this.hit = function() {
    this.hitsLeft--;
    
    if (this.hitsLeft <= 0) {
      deleteElement(this.id);
      var indexOfThisCircle;
      
      for (var i=0; i<circles.length; i++) {
        if (circles[i].id == this.id) {
          indexOfThisCircle = i;
        }
      }
      
      circles.splice(indexOfThisCircle, 1);
      
      return true;
    } else {
      this.update();
      
      return false;
    }
  };
  
  /*
  var discriminant = -b*b - 2*b*h*m + 2*b*k - h*h*m*m + 2*h*k*m - k*k + m*m*r*r + r*r;
  var x1 = (-Math.sqrt(discriminant) - b*m + h + k*m)/(m*m + 1);
  var x2 = (Math.sqrt(discriminant) - b*m + h + k*m)/(m*m + 1);
  */
  
  this.delete = function() {
    deleteElement(this.id);
  };
  
  this.update = function () {
    var url;
    if (this.hitsLeft == 1) {
      url = "circle1.png";
    } else if (this.hitsLeft == 2) {
      url = "circle2.png";
    } else if (this.hitsLeft == 3) {
      url = "circle3.png";
    } else {
      throw "hmm?";
    }
    
    setImageURL(this.id, url);
  };
  
  
  this.update();
  
  var that = this;
  
  var frames = 10;
  var dRadius = this.r/frames;
  var currentRadius = 0;
  var animationLoop = timedLoop(20, function() {
    setPosition(that.id, that.x-currentRadius, that.y-currentRadius, 2*currentRadius, 2*currentRadius);

    currentRadius += dRadius;

    frames--;
    if (frames < 0) stopTimedLoop(animationLoop);
  });
}

function Shot(x0, y0, v0, angle) {
  this.x = x0;
  this.y = y0;
  this.v = v0;
  this.angle = angle;
  
  this.slopeIntercept = function() {
    // y = mx + b
    // this.y = Math.tan(this.angle)*this.x + b
    var slope = Math.tan(this.angle);
    var y_int = this.y - slope*this.x;
    
    return [slope, y_int];
  };
  
  this.step = function () {
    var newX = this.x + this.v*Math.cos(this.angle);
    var newY = this.y - this.v*Math.sin(this.angle);
    
    if (newX + SHOT_RADIUS > 320 || newX - SHOT_RADIUS < 0) {
      this.angle = TAU/2 - this.angle;
    }
    if (newY + SHOT_RADIUS > 450 || newY - SHOT_RADIUS < 0) {
      this.angle = TAU - this.angle;
    }
    
    for (var i=0; i<circles.length; i++) {
      var circle = circles[i];
      if (distance(newX, newY, circle.x, circle.y) < SHOT_RADIUS + circle.r) {
        // we collided with the circle
        if (circle.hit()) {
          score++;
          setText("scoreLabel", "Score: "+score);
        }
        
        
        var angleWithCenter = Math.atan2(newY-circle.y, circle.x-newX);
        var theta2 = angleWithCenter + TAU/4;
        theta2 %= TAU;
        if (theta2 < 0) theta2 += TAU;
        
        this.angle = 2*theta2 - this.angle;
      }
    }
    
    var vx = this.v*Math.cos(this.angle);
    var vy = this.v*Math.sin(this.angle);
    this.x += vx;
    this.y -= vy;
    
    for (i=0; i<circles.length; i++) {
      var circle = circles[i];
      var angleToCircle = Math.atan2(this.y-circle.y, circle.x-this.x);
      var distanceToCircle = distance(this.x, this.y, circle.x, circle.y);
    
      // newton's equations
      var acc = GRAVITY_STRENGTH * circle.r*circle.r / (distanceToCircle*distanceToCircle);
      vx += acc * Math.cos(angleToCircle);
      vy += acc * Math.sin(angleToCircle);
    }
    
    this.angle = Math.atan2(vy, vx);
    this.v = Math.sqrt(vy*vy + vx*vx);
    this.v -= 0.15;
    //this.v *= 0.980;
    
    setPosition("shot", this.x-SHOT_RADIUS, this.y-SHOT_RADIUS);
  };
}

function createBackground() {
  createCanvas("background", 320, 450);
  setActiveCanvas("background");
  setStrokeWidth(5);
  setStrokeColor("black");
  line(0, 450-BARRIER_DISTANCE, 320, 450-BARRIER_DISTANCE);
}

function createShot() {
  createCanvas("shot", 2*SHOT_RADIUS, 2*SHOT_RADIUS);
  setActiveCanvas("shot");
  hideElement("shot");
  setStrokeColor(rgb(0,0,0, 0));
  setFillColor("black");
  
  circle(SHOT_RADIUS, SHOT_RADIUS, SHOT_RADIUS);
}


function normalize(angle) {
  angle %= TAU;
  if (angle < 0) angle += TAU;
  return angle;
}

function distance(x0, y0, x1, y1) {
  var dx = x1 - x0;
  var dy = y1 - y0;
  
  return Math.sqrt(dx*dx + dy*dy);
}

function uniqueId() {
  return "#"+randomNumber(0, 9999999999);
}


