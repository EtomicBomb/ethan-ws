// constants, for some reason `const` doesn't work
//var MINE = "https://d30y9cdsu7xlg0.cloudfront.net/png/915944-200.png";

// use left click to reveal the tiles, and middle click to flag the mines
// the goal is to flag all of the mines


var FLAG = "icon://fa-flag";
var SQUARE = "icon://fa-stop";

var HEIGHT = 10;
var WIDTH = 8;
var FLAGS = 10;

// colors of the numbers 
var COLORS = [null, "blue", "green", "red", "darkblue", "#800401", "#067F80", "black", "#7F7F7F"];

var lastDeleted = 0;
var idState = 0;

// game state
var mines;
var numbers;
var flags;
var revealed;
var flagsLeft;
var wrongFlags;
var startTime;


onEvent("game", "mousedown", function(event) {
  // clicking on the screen
  // for revealing, placing a flag/similar
  var grid = getGridSquare(event.x, event.y);
  if (!grid) {
    // we don't want to do anything if they click on the top buttons
    return;
  }
  
  var x = grid[0];
  var y = grid[1];
  
  if (event.button === 0) {
    // left click, reveal   

    var bound = 0; // add that one cool clicky feature
    if (revealed[y][x]) {
      bound = 1;
      
      // make sure that there are no mines
      
    }
    
    for (var h = -bound; h <= bound; h++) {
      for (var k = -bound; k <= bound; k++) {
        if (inBounds(x+h, y+k) && !flags[y+k][x+h]) {
          if (mines[y+k][x+h]) {
            // you've just clikced a mine
            setScreen("lose");
            return;
          } else {
            floodFill(x+h, y+k);
          }
        }
      }
    }
    
  } else {
    // middle click, flag
    if (!revealed[y][x]) {
      flag(x, y);
    }
  }
});

// stupid handlers
// win buttons
onEvent("winTitle", "click", function() {
  setScreen("title");
});
// lose buttons
onEvent("loseTitle", "click", function() {
  setScreen("title");
});
onEvent("loseRules", "click", function() {
  setScreen("rules");
});
// rules buttons
onEvent("rulesPlay", "click", function() {
  newGameState();
  setScreen("game");
});
onEvent("rulesTitle", "click", function() {
  setScreen("title");
});
// title buttons
onEvent("titlePlay", "click", function() {
  newGameState();
  setScreen("game");
});
onEvent("titleRules", "click", function() {
  setScreen("rules");
});
// game buttons
onEvent("gameTitle", "click", function() {
  setScreen("title");
});
onEvent("gameRules", "click", function() {
  setScreen("rules");
});

function flag(x, y) {
  if (flags[y][x]) {
    // remove flag
    flags[y][x] = false;
    drawTanSquare(x, y);
    
    if (mines[y][x]) {
      flagsLeft++;
    } else {
      wrongFlags--;
    }
    
  } else {
    // place flag
    flags[y][x] = true;
    drawBlankSquare(x, y);
    drawFlag(x, y);
    
    if (mines[y][x]) {
      flagsLeft--;
    } else {
      wrongFlags++;
    }
  }
  
  // check if you've won
  if (flagsLeft === 0 && wrongFlags === 0) {
    setScreen("win");
    var score = Math.round((getTime() - startTime)/1000);
    
    setText("score", score.toString());
  }  
}


function floodFill(x, y) {
  if (revealed[y][x]) {
    return;
  } 
  
  revealed[y][x] = true;
  drawBlankSquare(x, y);
  
  var n = numbers[y][x];
  if (n !== 0) {
    drawNumber(n, x, y);
    
    return;
  }
  
  // because it's not zero, we can fill around it
  for (var h = -1; h <= 1; h++) {
    for (var  k = -1; k <= 1; k++) {
      if (inBounds(x+h, y+k)) {
        floodFill(x+h, y+k);
      }
    }
  }
  
}

function newGameState() {
  // clear all objects from the screen
  for (var i = lastDeleted; i < idState; i++) {
    deleteElement(i.toString());
  }
  lastDeleted = idState;
  
  var board = getBoard();
  mines = board[0];
  numbers = board[1];
  flags = blankMatrixBool();
  revealed = blankMatrixNum();
  startTime = getTime();
  
  flagsLeft = FLAGS;
  wrongFlags = 0;
  
}

function getBoard() {
  // returns a booleon array, and array of how close
  // you are to a mine
  var mines = blankMatrixBool();
  var numbers = blankMatrixNum();
  
  // put the mines in random places
  for (var i = 0; i < FLAGS; i++) {
    var x = randomNumber(0, WIDTH-1);
    var y = randomNumber(0, HEIGHT-1);
    
    if (mines[y][x]) {
      // we want exactly count mines
      // run this loop one more time
      i--;
    } else {
      mines[y][x] = true;
      for (var h = -1; h <= 1; h++) {
        for (var k = -1; k <= 1; k++) {
          if (inBounds(x+k, y+h)) {
            numbers[y+h][x+k]++;
            
          }
        }
      }
    }
  }
  
  return [mines, numbers];
}

function blankMatrixNum() {
  var matrix = [];
  
  for (var y = 0; y < HEIGHT; y++) {
    var row = [];
    for (var x = 0; x < WIDTH; x++) {
      appendItem(row, 0);
    }
    appendItem(matrix, row);
  }
  
  return matrix;
}

function blankMatrixBool() {
  var matrix = [];
  
  for (var y = 0; y < HEIGHT; y++) {
    var row = [];
    for (var x = 0; x < WIDTH; x++) {
      appendItem(row, false);
    }
    appendItem(matrix, row);
  }
  
  return matrix;
}

function inBounds(x, y) {
  // just to make things simpler on me
  return x >= 0 && x < WIDTH && y >= 0 && y < HEIGHT;
}

function getGridSquare(x, y) {
  // takes x and y of the cursor, returns
  // the grid square that the cursor is in
  
  var gridX = Math.floor((x-2)/39.52);
  var gridY = Math.floor((y-52)/39.45);
  
  if (gridX < 0 || gridX >= WIDTH || gridY < 0 || gridY >= HEIGHT) {
    return null;
  }
  
  return [gridX, gridY];
}

function newSprite(url, width, height) {
  // returns ID representing a new image, and creates it
  var id = idState.toString();
  idState++; // make sure each sprite has a unique id
  
  image(id, url);
  hideElement(id);
  setSize(id, width, height);
  
  return id;
}

function drawSprite(id, x, y) {
  // makes the sprite visible
  setPosition(id, x, y);
  showElement(id);
}

// RESTORATION:
const smallerSquare = 0.8;
function drawBlankSquare(x, y) {
  var id = newSprite('white-square.svg', 57, 67);
  document.getElementById(id).style.transform = 'scale(0.7)';
  putSquare(id, x, y);
}

function drawTanSquare(x, y) {
  var id = newSprite('tan-square.svg', 57, 67);
  document.getElementById(id).style.transform = 'scale(0.7)';
  putSquare(id, x, y);
}

function drawFlag(x, y) {
  var id = newSprite('red-flag.svg', 40, 40);
  document.getElementById(id).style.transform = 'scale(0.7)';
  drawSprite(id, 39.52*x-0, 53.45+39.45*y);
}

function putSquare(id, x, y) {
  drawSprite(id, 39.52*x-6.83, 40.45+39.45*y);
}
//function drawBlankSquare(x, y) {
//  var id = newSprite(SQUARE, 57, 67);
//  putSquare(id, x, y, "white");
//  
//}
//
//function drawTanSquare(x, y) {
//  var id = newSprite(SQUARE, 57, 67);
//  putSquare(id, x, y, "#ffd4b8");
//  
//}
//
//function drawFlag(x, y) {
//  var id = newSprite(FLAG, 40, 40);
//  setProperty(id, "icon-color", "red");
//  
//  drawSprite(id, 39.52*x-0, 53.45+39.45*y);
//  
//}
//
//function putSquare(id, x, y, color) {
//  // moves the square to the desired location
//  
//  // y=39.52380952381x-6.8333333333333
//  // y=40.454545454545+39.454545454545x
//  setProperty(id, "icon-color", color);
//  
//  drawSprite(id, 39.52*x-6.83, 40.45+39.45*y);
//}

function drawNumber(n, x, y) {
  if (n === 0) {
    // early return because we don't draw 0's
    return null;
  }
  var id = idState.toString();
  idState++;
  
  textLabel(id, n.toString());
  hideElement(id);
  
  setPosition(id, 39.52*x+10.17, 50.45+39.45*y);
  setProperty(id, "font-size", 40);
  setProperty(id, "text-color", COLORS[n]);  
  
  showElement(id);
  return id;
}
