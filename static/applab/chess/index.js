/* jshint eqnull:true loopfunc:true */
var SPRITE_SIZES = {whiteKing:[40,40], whiteQueen:[40,40], whiteRook:[40,40], whiteBishop:[40,40], whiteKnight:[40,40], whitePawn:[40,40], blackKing:[40,40], blackQueen:[40,40], blackRook:[40,40], blackBishop:[40,40], blackKnight:[40,40], blackPawn:[40,40]};

var isTrackingPiece = false;
var pieceToTrackStartX;
var pieceToTrackStartY;


var schedule = new WriteSchedule();
var broadcast;

var unaccountedMessages = 0; // if this is negative, somebody has tampered with our record

var joinGameButtons = [];

var listenForNewGames = false;
var listenForGuestJoin = false;

var gameRecordServerCopy;
var gameRecordPlayerCopy;

var board;

var myName;
var otherName;
var myGroup;
var myColor;
var isMyTurn;

var pingMean = new LastNMean(5, 1500);

var gameOver = false;



function broadcastHandler(txer, data) {
  setText("pingLabel", "Ping: "+ Math.floor(pingMean.read()/2));
  
  if (data.type == "move") {
    if (gameOver) {
        return;
    }

    var result = board.tryHardMove(data.start.x, data.start.y, data.end.x, data.end.y, txer);
    if (!result) {
        board.correctPosition(data.start.x, data.start.y); // snap the piece back
    }

  } else if (data.type == "chat") {
    setText("chatOutput", txer+": "+data.content+"\n"+getText("chatOutput"));
  }
}

function setTurnLabel() {
  var text;
  
  if (isMyTurn) {
    text = myColor +"/"+ myName;
  } else {
    text = colorOf(otherName) +"/"+ otherName;
  }
  
  setText("whosTurn", "Turn: "+text);
}

function colorOf(playerName) {
  if (playerName == myName) {
    return myColor;
  } else if (myColor == "white") {
    return "black";
  } else {
    return "white";
  }
}

onEvent("chatInput", "keydown", function(event) {
  var toSend = getText("chatInput");
  if (event.key == "Enter" && toSend.length > 0) {
    setText("chatInput", "");
    
    broadcast.tx({
      type: "chat",
      content: toSend,
    });
  }
});


//////////////////////////////////CREATE THE GAME STATE////////////////////////////



function initGame() {
  // check if we can log into a game
  listenForGuestJoin = false;
  myColor = myName == myGroup ? "white" : "black";
  otherName = myName == myGroup ? gameRecordPlayerCopy.guest : gameRecordPlayerCopy.owner;
  isMyTurn = myColor == "white";
  
  
  var rng = new Rng(gameRecordPlayerCopy.seed);
  // figure out who is white and who is black
  
  drawBackground();
  setTurnLabel();
  
  setScreen("main");
//  createCanvas("background", 320, 450);

  
  board = new Board(rng);

  broadcast = new Broadcast(myName, myGroup);
}



///////////////////////////////////CHESS PIECES///////////////////////////////////


onEvent("main", "mousedown", function(event) {
  if (isTrackingPiece) return;
  
  // figure out which square was targeted
  
  var pos = screenToMatrix(event.x, event.y);
  if (pos == null) return;
  
  isTrackingPiece = true;
  pieceToTrackStartX = pos.x;
  pieceToTrackStartY = pos.y;
});

onEvent("main", "mousemove", function(event) {
  if (!isTrackingPiece) return;
  board.softMove(pieceToTrackStartX, pieceToTrackStartY, event.x, event.y);
});

onEvent("main", "mouseup", function(event) {
  if (!isTrackingPiece) return;
  isTrackingPiece = false;
  
  var newGridPos = screenToMatrix(event.x, event.y);
  
  // send the signal to move the piece
  broadcast.tx({
    type: "move",
    start: {
      x: pieceToTrackStartX,
      y: pieceToTrackStartY,
    },
    end: {
      x: newGridPos.x,
      y: newGridPos.y,
    },
  });
});




function Board(rng) {
  this.matrix;
  
//  this.tryHardMove = function(oldX, oldY, newX, newY) {
  this.tryHardMove = function(oldX, oldY, newX, newY, txer) {
    var wrongMover = txer == myName ? !isMyTurn : isMyTurn;
    var oldOutOfBounds = oldX<0 || oldY<0 || oldX>7 || oldY>7;
    var newOutOfBounds = newX<0 || newY<0 || newX>7 || newY>7;
    var samePosition = oldX == newX && oldY == newY;
    if (wrongMover || oldOutOfBounds || newOutOfBounds || samePosition) {
        return false;
    }
    var pieceToMove = this.matrix[oldY][oldX];
    if (pieceToMove == null || pieceToMove.color != colorOf(txer)) {
        return false;
    }

    var shouldMoveFunc = eval(pieceToMove.type.toLowerCase()+"CanMove");
    var shouldMove = shouldMoveFunc(oldX, oldY, newX, newY, this.matrix, pieceToMove.color);
    
    if (!shouldMove) {
        return false;
    }
    
    if (this.matrix[newY][newX] != null) {
      // capture
      if (this.matrix[newY][newX].type == "King") {
        gameOver = true;
        // figure out the winning player
        var losingColor = this.matrix[newY][newX].color;
        var winningPlayer = losingColor != myColor ? myName : otherName;
        setText("whoWon", winningPlayer + " Won!");
        setText("whosTurn", "");
        throw new Error("Game over.");
      }
      
      this.matrix[newY][newX].delete(); // capture the old piece
      this.matrix[newY][newX] = null;
    }
    
    // now that that space is freed, we can put the new piece there
    this.matrix[newY][newX] = this.matrix[oldY][oldX];
    this.matrix[oldY][oldX] = null;
    
    pieceToMove.setMatrixPosition(newX, newY);
    this.correctPosition(newX, newY);
    
    isMyTurn = !isMyTurn;
    setTurnLabel();
    return true;
  };
  
  this.softMove = function(oldMatrixX, oldMatrixY, newScreenX, newScreenY) {
    // only moves the sprite
    var pieceToMove = this.matrix[oldMatrixY][oldMatrixX];
    if (pieceToMove == null) return false;
    pieceToMove.setScreenPosition(newScreenX-20, newScreenY-20);
  };
  
  this.correctPosition = function(x, y) {
    // snaps the piece at (x, y) back to the grid
    var outOfBounds = x<0 || y<0 || x>7 || y>7;
    if (outOfBounds) return;
    var piece = this.matrix[y][x];
    if (!piece) return;
    piece.correctSpritePosition();
  };
  
  this.makeMatrix = function(rng) {
    this.matrix = [];
    for (var i=0; i<8; i++) {
      var row = [];
      for (var j=0; j<8; j++) {
        row.push(null);
      }
      this.matrix.push(row);
    }
    
    // do rooks, knights and bishops
    var names = ["Rook", "Knight", "Bishop"];
    for (var x1=0; x1 < 3; x1++) {
      var x2 = 7-x1;
      var name = names[x1];

      this.matrix[7][x1] = new Piece(x1, 7, name, "white");
      this.matrix[7][x2] = new Piece(x2, 7, name, "white");
      
      this.matrix[0][x1] = new Piece(x1, 0, name, "black");
      this.matrix[0][x2] = new Piece(x2, 0, name, "black");
    }
    
    // queen
    this.matrix[7][3] = new Piece(3, 7, "Queen", "white");
    this.matrix[0][4] = new Piece(4, 0, "Queen", "black");
    
    // kings
    this.matrix[7][4] = new Piece(4, 7, "King", "white");
    this.matrix[0][3] = new Piece(3, 0, "King", "black");

    
    // put pawns in
    for (var x=0; x<8; x++) {
      // white pieces
      this.matrix[1][x] = new Piece(x, 1, "Pawn", "black");
      this.matrix[6][x] = new Piece(x, 6, "Pawn", "white");
    }
  };
  
  this.makeMatrix(rng);
}

function screenPosition(x, y) {
  if (myColor == "white") {
    return {
      x: 40*x,
      y: 130+40*y,
    };
  } else if (myColor == "black") {
    return {
      x: 40*(7-x),
      y: 130+40*(7-y),
    };
  }
}

function screenToMatrix(screenX, screenY) {
  if (screenY < 130) return null;
  
  var 
    absoluteX = Math.floor(screenX/40),
    absoluteY = Math.floor((screenY-130)/40),
    matrixX = myColor == "white" ? absoluteX : 7-absoluteX,
    matrixY = myColor == "white" ? absoluteY : 7-absoluteY;
    
  return {
    x: matrixX,
    y: matrixY,
  };
}

function randomPiece(x, y, color, rng) {
  var n = rng.range(1, 5); 
  if (n == 1) {
    return new Piece(x, y, "Rook", color);
  } else if (n == 2) {
    return new Piece(x, y, "Bishop", color);
  } else if (n == 3) {
    return new Piece(x, y, "Queen", color);
  } else if (n == 4) {
    return new Piece(x, y, "Knight", color);
  } else if (n == 5) {
    return new Piece(x, y, "Pawn", color);
  }
}

function Piece(x, y, type, color) {
  this.x = x;
  this.y = y;
  this.type = type;
  this.color = color;
  
  this.init = function() {
    this.sprite = new Sprite(this.color+this.type);
    this.correctSpritePosition();
    this.sprite.show();
  };
  
  this.delete = function() {
    this.sprite.delete();
  };
  
  this.setScreenPosition = function(newX, newY) {
    // moves the visual position of the sprite, not the actual coords
    this.sprite.setPosition(newX, newY);
  };
  
  this.setMatrixPosition = function(newX, newY) {
    // sets the actual stored position
    this.x = newX;
    this.y = newY;
  };
  
  this.correctSpritePosition = function() {
    // move the sprite to the actual coordinates stored in piece
    var screenPos = screenPosition(this.x, this.y);
    this.sprite.setPosition(screenPos.x, screenPos.y);
  };
  
  this.init();
}

function kingCanMove(x, y, newX, newY, matrix, color) {
  return isTouching(x, y, newX, newY);
}

function rookCanMove(x, y, newX, newY, matrix, color) {
  return isStraitFrom(x, y, newX, newY) && !objectInPath(x, y, newX, newY, matrix);
}

function bishopCanMove(x, y, newX, newY, matrix, color) {
  return isDiagonalFrom(x, y, newX, newY) && !objectInPath(x, y, newX, newY, matrix);
}

function queenCanMove(x, y, newX, newY, matrix, color) {
  return (isDiagonalFrom(x, y, newX, newY) || isStraitFrom(x, y, newX, newY)) && 
        !objectInPath(x, y, newX, newY, matrix);
}

function knightCanMove(x, y, newX, newY, matrix, color) {
  return distance(x, y, newX, newY) == Math.sqrt(5);
}

function pawnCanMove(x, y, newX, newY, matrix, color) {
  var regularMove;
  if (color == "white") {
    // left side of the board
    regularMove = x == newX && 
      (y-1 == newY || (y-2 == newY && y == 6));
  } else if (color == "black") {
    regularMove = x == newX &&
      (y+1 == newY || (y+2 == newY && y == 1));
  }
  
  if (regularMove) {
    // we are doing a regular move forwards
    return !matrix[newY][newX];

  } else {
    // try doing a side capture
    var newInFront = color == "white" ? newY < y : newY > y;
    console.log(distance(x, y, newX, newY) == Math.sqrt(2));
    console.log(newInFront);
    console.log(matrix[newY][newX])
    return newInFront && matrix[newY][newX] && distance(x, y, newX, newY) == Math.sqrt(2);
  }
}



function objectInPath(x1, y1, x2, y2, matrix) {
  // we only have to worry about strait lines or diagonals
  // points 1 and 2 are guarunteed to not be in the same place

  var dx = x2-x1 === 0 ? 0 : (x2-x1)/Math.abs(x2-x1);
  var dy = y2-y1 === 0 ? 0 : (y2-y1)/Math.abs(y2-y1);
  
  x1 += dx; // do it to start off with so that we don'tget a false positive on point 1
  y1 += dy;
  
  while (x1 != x2 || y1 != y2) {
    if (matrix[y1][x1]) return matrix[y1][x1];
    
    x1 += dx;
    y1 += dy;
  }
  
  return null;
}


function isDiagonalFrom(x1, y1, x2, y2) {
  if (x2-x1 === 0) {
    return false;
  }
  
  var slope = (y2-y1)/(x2-x1);
  return Math.abs(slope) == 1;
}

function isTouching(x1, y1, x2, y2) {
  var dist = distance(x1, y1, x2, y2);
  return dist == 1 || dist == Math.sqrt(2);
}

function isStraitFrom(x1, y1, x2, y2) {
  return x1 == x2 || y1 == y2;
}

function Sprite(name, x, y, angle) {
  this.x = x == null ? 0 : x;
  this.y = y == null ? 0 : y;
  this.angle = angle == null ? 0 : angle;
  this.width = SPRITE_SIZES[name][0];
  this.height = SPRITE_SIZES[name][1];
  
  this.id;
  
  this.show = function() {
    showElement(this.id);
  };
  this.hide = function() {
    hideElement(this.id);
  };
  this.delete = function() {
    deleteElement(this.id);
    
  };
  
  this.getPosition = function() {
    return {x:this.x, y:this.y};
  };

  this.setPosition = function(newX, newY) {
    this.x = newX;
    this.y = newY;
    setPosition(this.id, this.x, this.y);
  };

  this.getAngle = function() {
    return this.angle;
  };
  
  this.setAngle = function(newAngle) {
    this.angle = newAngle; 
    setStyle(this.id, "transform: rotate("+ (-this.angle) +"rad);");
  };
  
  this.init = function(name) {
    this.id = uniqueID().toString();
    
    button(this.id, "");
    this.hide();
    setProperty(this.id, "background-color", "#00000000");
    setProperty(this.id, "image", name+".png");


    setStyle(this.id, "z-index: 999");
    setSize(this.id, this.width, this.height); // the size of an image defaults to null
    
    this.setAngle(this.angle);
    this.setPosition(this.x, this.y);
  };

  this.init(name);
}

function distance(x0, y0, x1, y1) {
  var dx = x1-x0;
  var dy = y1-y0;
  return Math.sqrt(dx*dx+dy*dy);
}

function drawBackground() {
  setActiveCanvas("background");
  setStrokeColor("#00000000");
  setFillColor("tan");
  circle(0, 0, 5000);
  setFillColor("black");
  
  var shouldDraw = true;
  
  for (var x=0; x<320; x += 40) {
    for (var y=130; y<450; y += 40) {
      
      if (shouldDraw) rect(x, y, 40, 40);
      
      shouldDraw = !shouldDraw;
    }
    shouldDraw = !shouldDraw;
  }
}



//////////////////////////////////MULTIPLAYER CODE//////////////////////////////////

// CHANGES: remove startGame screen
//onEvent("gotoStartGameScreen", "click", function() {
//  setScreen("startGame");
//});

onEvent("gotoJoinGameScreen", "click", function() {
  drawJoinGameScreen();
  listenForNewGames = true;
  setScreen("joinGame");
});

//onEvent("difficultySlider", "input", function() {
//  setText("difficultySliderLabel", getNumber("difficultySlider"));
//});

//onEvent("startGameButton", "click", function() {
onEvent("gotoStartGameScreen", "click", function() {
  myName = getUsername();
  createRecord("newtable", {}, function() {});
  readRecords("games", {}, function(records) {
    var willStartGame = true;    
    for (var i=0; i<records.length; i++) {

      if (records[i].owner == myName) {
        willStartGame = false;
        setText("startGameError", "There is already a game owned by a player with your username.");
      } else if (records[i].deadman + 10000 < getTime()) {
        // this is an inactive game, purge everything related to it
        schedule.add("delete", "games", records[i]);
        var ownerToDelete = records[i].owner;
        var guestToDelete = records[i].guest;
        readRecords("broadcasts", {}, function(beans) {
          // some beans are good, others are wormy
          for (var j=0; j<beans.length; j++) {
            if (beans[j].txer == ownerToDelete || beans[j].txer == guestToDelete) {
              schedule.add("delete", "broadcasts", beans[j]);
            }
          }
        });
      }
    }
    
    if (willStartGame) {
      // we're all good to go
      var gameRecordInit = {
        seed: randomNumber(1, 100000), // random seed for planet generation
        owner: myName,
        guest: null,
        deadman: getTime(), // we have a deadman switch to delete the game
//        hardness: getNumber("difficultySlider"),
        // when the player logs out. When the player creates a record, we try to delete
      };
      
      schedule.add("create", "games", gameRecordInit, function(record) {
        gameRecordServerCopy = record;
    
        timedLoop(5000, function() {
          // communicate that the game is still active
          gameRecordServerCopy.deadman = getTime();
          schedule.add("update", "games", gameRecordServerCopy);
        });
        
        myGroup = myName;
        gameRecordPlayerCopy = record;
        listenForGuestJoin = true;
        
        setScreen("lobby");
      });
    }
  });
});


function drawJoinGameScreen() {
  readRecords("games", {}, function(records) {
    // add all of these games to the dropdown
      
    for (var i=0; i<joinGameButtons.length; i++) {
      deleteElement(joinGameButtons[i]);
    }
      
    var dispIndex = 0;
    for (i=0; i<records.length; i++) {
      if (records[i].guest != null || records[i].deadman+10000 < getTime()) continue;
      dispIndex++;
      
      var id = uniqueID().toString();
      joinGameButtons.push(id);
      
      var owner = records[i].owner;
      button(id, dispIndex+". "+ owner);
      
      setPosition(id, 0, 30*i+100, 320, 30);
      
      onEvent(id, "click", function() {
        tryJoinGame(owner);
      });
    }
  });
}

function tryJoinGame(owner) {
  // TODO: disable the other buttons
  readRecords("games", {owner:owner}, function(records) {
    myName = getUsername();
    if (records.length != 1) {
      setText("joinGameError", "Wrong number of games by that name exist.");
    } else if (records[0].owner == myName || records[0].guest == myName) {
      setText("joinGameError", "A player in that game has your name.");
    } else if (records[0].guest != null) {
      setText("joinGameError", "That game is full.");
    } else {
      var tryRecord = records[0];
      tryRecord.guest = myName;
      listenForNewGames = false;
      
      schedule.add("update", "games", tryRecord, function(newRecord) {
        gameRecordPlayerCopy = newRecord;
        myGroup = owner;
        
        initGame();
      });
    }
  });
}


onRecordEvent("games", function(record, type) {
  if (gameRecordServerCopy != null && gameRecordServerCopy.id == record.id) {
    // we are a server and we want to keep a copy of the game record as it update
    gameRecordServerCopy = record;
  }
  
  if (listenForGuestJoin && record.id == gameRecordPlayerCopy.id && record.guest != null) {
    gameRecordPlayerCopy.guest = record.guest;
    
    initGame();
    listenForGuestJoin = false;
  }

  if (listenForNewGames && type == "create") {
    drawJoinGameScreen();
  }
});


onRecordEvent("broadcasts", function(record, type) {
  if (type == "create" || type == "delete" || record.group != myGroup) return; 
  
  if (record.txer == myName) {
    unaccountedMessages--;
    if (unaccountedMessages < 0) panic();
  } 
  
  if (record.sentTime != null) {
    // we also want to record the average ping
    pingMean.add(getTime()-record.sentTime);
  }

  broadcastHandler(record.txer, JSON.parse(record.data));
});



function Broadcast(txer, group) {
  this.buf = [];
  this.record;
  
  this.tx = function(data) {
    if (this.record == null) {
      // the record isn't created immediately, what if items are added too quickly
      this.buf.push(data);
      return;
    }

    this.record.sentTime = getTime();    
    this.record.data = JSON.stringify(data);
    unaccountedMessages++;
    schedule.add("update", "broadcasts", this.record);
  };
  
  this.init = function(txer, group) {
    var initialRecord = {txer:txer, group:group};
    var that = this;
    
    unaccountedMessages++;
    schedule.add("create", "broadcasts", initialRecord, function(record) {
      that.record = record;
      
      while (that.buf.length > 0) {
        var item = that.buf.pop();
        that.tx(item);
      }
    });
  };
  
  this.init(txer, group);
}


function WriteSchedule() {
  this.buf = [];
  this.lastUpdated = 0;
  
  this.add = function(type, table, record, callback) {
    if (callback == null) callback = doNothing;

    record = JSON.parse(JSON.stringify(record)); // we want to do a deep copy
    
    var f;
    if (type == "create") {
      f = createRecord;
    } else if (type == "update") {
      f = updateRecord;
    } else if (type == "delete") {
      f = deleteRecord;
    }
    
    
    this.buf.push(function() { f(table, record, callback) });
    this.update();
  };
  
  this.update = function() {
    if (this.lastUpdated+200 > getTime() || this.buf.length === 0) return;

    this.buf.pop()(); // call the function
    this.lastUpdated = getTime();
  };
  
  var that = this;
  timedLoop(100, function() {
    that.update();
  });
}

function LastNMean(n, init) {
  var last = [];
  var mean;
  
  this.read = function() {
    return mean;
  };
  
  this.add = function(value) {
    last.push(value);
    if (last.length > n) {
      last.shift();
    }
    
    // calculate the mean here so this.read can be cheap
    var sum = 0;
    for (var i=0; i<last.length; i++) sum += last[i];
    mean = sum / last.length;  
  };
  
  this.add(init);
}

function Rng(seed) {
  // generate seeded random numbers
  // adapted from https://stackoverflow.com/a/1930372
  
  this.range = function(min, max) {
    max++; // so it will take inclusive ranges
    return Math.floor(this.fraction() * (max-min) + min);
  };
  
  this.fraction = function() {
    var x = Math.sin(seed++) * 10000;
    return x - Math.floor(x);
  };
}


function doNothing() {}

function panic() {
  setScreen("panic");
}


function uniqueID() {
  return randomNumber(0, 99999999);
}

function getUsername() {
  return getText("usernameInput");
}
