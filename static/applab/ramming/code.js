var TAU = 2*Math.PI;
var COLOR_CLEAR = rgb(0, 0, 0, 0);
var SECONDS_PER_TURN = 15;

var SCREEN_WIDTH = 320;
var SCREEN_HEIGHT = 450;


var myRecord = null;
var createdGameRecord = null;
var gameRecord = null;

var shouldRefreshDropdown = false;
var shouldRefreshLobby = false;
var listenForBegin = false; 
var hasBegun = false;

var isOwner = null;

var backgroundIndex = 1;
var chatIsShown = false;

var schedule = new WriteSchedule();

var listenForBroadcast = false;

var client = null;



function Client(seed, team) {
  // pointer api stuff
  this.pointer = null;
  this.onPointerClick = null;
  
  this.createPointer = function (initialPos, constructor, onPointerClick) {
    this.pointer = this.sprites.addSprite(constructor, initialPos[0], initialPos[1]);
    this.onPointerClick = onPointerClick;
  };
  
  this.sendMessage = function (message) {
    broadcast({
      kind: "chat",
      message: message,
    });
  };
  
  this.requestBoatMove = function (id, newPos) {
    broadcast({
      kind: "move",
      id: id,
      newPos: newPos,
    });
  };
  
  this.requestBackUp = function (id) {
    broadcast({
      kind: "backUp",
      id: id,
    });
  };

  
  this.rng = new Rng(seed);
  
  this.sprites = new Sprites(this.randomSeed);
  this.toolbar = new Toolbar();
  this.action = new Action(gameRecord.redPlayers, gameRecord.bluePlayers, gameRecord.beginTime);

  this.chatText = "";
  
  var that = this;
}

function Action(redPlayers, bluePlayers, startTime) {
  // tracks who's turn it currently is
  // all red players, go, then blue palyers
  // TODO: add multiple moves per turn
  // for now, lets just have one move per turn. maybe it will be more fast paced that way
  var nRedPlayers = redPlayers.length;
  var nBluePlayers = bluePlayers.length;
  
  var baseActionPoints = nRedPlayers*20;

  this.teamActionPoints = baseActionPoints;

  this.turnStartTime = startTime;

  this.turnIndex = 0; // is an index in the list. if there are 2 players on each team, this will be zero per cycle
  this.isRedsTurn = true;

  this.currentPlayersName = function () {
    if (this.isRedsTurn) {
      return redPlayers[this.turnIndex];
    } else {
      return bluePlayers[this.turnIndex];
    }
  };
  
  this.currentTeam = function () {
    return this.isRedsTurn? "red" : "blue";
  };

  this.endTurn = function (turnEndTime) {
    this.turnIndex++;
    this.turnStartTime = turnEndTime; // we're starting a new turn remember
    
    if (this.isRedsTurn && this.turnIndex == nRedPlayers) {
      // it is now the blue team's turn 
      // the team switched, so we want to reset the action points
      this.isRedsTurn = false;
      this.turnIndex = 0;
      
      this.teamActionPoints = baseActionPoints;
    } 
    if (!this.isRedsTurn && this.turnIndex == nBluePlayers) {
      this.isRedsTurn = true;
      this.turnIndex = 0;
      
      this.teamActionPoints = baseActionPoints;
    }
    
    // we have to do this stuffevery time the turn switches over
    this.displayTurnInfo();
    
    updatePlayerLabels();
  };
  
  this.displayTurnTime = function () {
    setText("timeLeft", Math.round((getTime()-this.turnStartTime)/1000) +" sec");
  };
  
  this.displayTurnInfo = function () {
    setProperty("whosTurn", "text-color", this.currentTeam());

    if (this.currentPlayersName() == myRecord.playerName) {
      setText("whosTurn", this.currentPlayersName() + "✓"); // we add the check so the player knows its them
      showElement("skipTurnButton");
    } else {
      setText("whosTurn", this.currentPlayersName());
      hideElement("skipTurnButton");
    }
    
    setProperty("actionPointsLabel", "text-color", this.currentTeam());
    setText("actionPointsLabel", this.teamActionPoints + " action points");

    this.displayTurnTime();
  };
  
  
  this.tryAction = function (player, broadcastRecord, timePerformed) {
    var kind = broadcastRecord.kind;
    var turnLength = timePerformed - this.turnStartTime;
    this.teamActionPoints -= Math.round(turnLength/1000);

    
    // returns true if the action is do-able
    if (kind == "chat") {
      var message = broadcastRecord.message;
      var oldChat = getText("chatOutput");
      // this doesn't use any aciton points or anything like that. it can just perform the action
      setText("chatOutput", player + ": " + message + "\n" + oldChat);
      
      if (chatIsShown && getChecked("aslCheckbox")) {
        aslInterpret(message);
      }
      
      
    } else if (kind == "move" && this.currentPlayersName() == player && client.sprites.spritesMap[broadcastRecord.id].team == this.currentTeam()) {
      // lets figure out how long the turn is
      client.sprites.spritesMap[broadcastRecord.id].moveTo(broadcastRecord.newPos);
      this.endTurn(timePerformed);
    } else if (kind == "backUp" && this.currentPlayersName() == player && client.sprites.spritesMap[broadcastRecord.id].team == this.currentTeam()) {
      client.sprites.spritesMap[broadcastRecord.id].backUp();
      this.endTurn(timePerformed);
    } else if (kind == "skipTurn" && this.currentPlayersName() == player) {
      this.endTurn(timePerformed);
    }
    
    // no matter what, we're gonna take away as many action points as there were seconds in the turn

    this.displayTurnInfo();
  };
  
  var that = this;
  
  // we want to display the turn info on the second
  // figure out how many milliseconds we are off of a full second
  this.displayTurnInfo();
  var millisecondShift = 1000 - (getTime()%1000);
  setTimeout(function() {
    timedLoop(1000, function() {
      that.displayTurnTime();
    });
  }, millisecondShift);
}

onEvent("skipTurnButton", "click", function() {
  // i don't really know a better place to put this
  broadcast({
    kind: "skipTurn",
  });
});

onEvent("main", "mousedown", function (event) {
  // the pointer was clicked, that means the pointer creator probably wants something to happen
  
  if (client.pointer) {
    var endPos = client.sprites.map.fromScreen(event.x, event.y);
    client.onPointerClick(endPos);
    
    // delete the pointer
    client.sprites.deleteSprite(client.pointer.id);
    client.pointer = null;
    client.onPointerClick = null;
  }
});

onEvent("main", "keydown", function(event) {
  if (client.pointer && event.key == "Esc") {
    // they are trying escape the pointer deelio
    // do the same thing as if they clicked, just don't do the callback
    client.sprites.deleteSprite(client.pointer.id);
    client.pointer = null;
    client.onPointerClick = null;
  }
});

onEvent("main", "mousemove", function (event) {
  if (client.pointer) {
    // we are tracking the 
    var localPos = client.sprites.map.fromScreen(event.x, event.y);
    client.pointer.setLocation(localPos[0], localPos[1]);
  }
});





function broadcastHandler(txer, data, sentTime) {
  // we recieved a broadcast, what should we do about it?
  // we should ask the turn 
  client.action.tryAction(txer, data, sentTime);
}

function broadcast(data) {
  myRecord.data = JSON.stringify(data);
  myRecord.sentTime = getTime();
  schedule.add("update", "players", myRecord, doNothing);
}

function beginGame(record) {
  gameRecord = record;
  gameRecord.redPlayers = JSON.parse(gameRecord.redPlayers);
  gameRecord.bluePlayers = JSON.parse(gameRecord.bluePlayers);
  
  listenForBegin = false;
  shouldRefreshLobby = false;
  listenForBroadcast = true;
  hasBegun = true;
  
  setScreen("main");

  client = new Client(record.randomSeed);
  
  // add the starter ships to the client
  // random positions
  var x, y, id, team, map, sprite;
  
  for (var i=0; i<gameRecord.redPlayers.length; i++) {
    x = client.rng.range(0, 320);
    y = client.rng.range(0, 450);
    id = "starterRed"+i;
    team = "red";
    map = client.sprites.map;
    
    sprite = new Carrier(x, y, id, map, team);
    client.sprites.addSpriteRaw(id, sprite);
  }
  
  for (i=0; i<gameRecord.bluePlayers.length; i++) {
    x = client.rng.range(0, 320);
    y = client.rng.range(0, 450);
    id = "starterBlue"+i;
    team = "blue";
    map = client.sprites.map;
    
    sprite = new Carrier(x, y, id, map, team);
    client.sprites.addSpriteRaw(id, sprite);
  }
}

onRecordEvent("players", function(record, type) {
  if (shouldRefreshLobby && record.creatorName == myRecord.creatorName) {
    refreshLobbyAnd(doNothing);
  }
  
  if (listenForBroadcast && record.creatorName == myRecord.creatorName && type == "update") {
    broadcastHandler(record.playerName, JSON.parse(record.data), record.sentTime);
  }
});


onRecordEvent("games", function(record, type) {
  if (shouldRefreshDropdown) {
    refreshGamesDropdownAnd(doNothing);
  }
  
  if (listenForBegin && record.creator == myRecord.creatorName && record.beginTime !== null) {
    beginGame(JSON.parse(JSON.stringify(record)));
  }
});

onEvent("gotoJoinGame", "click", function() {
  // just load up the join game screen dropdown
  refreshGamesDropdownAnd(function() {
    setScreen("joinGameScreen");
  });
});

onEvent("beginButton", "click", function() {
  // lets actually kick off the game and start displaying graphics/generating stuff
  readRecords("players", {creatorName:createdGameRecord.creator}, function(records) {
    var bluePlayers = [];
    var redPlayers = [];
    
    for (var i=0; i<records.length; i++) {
      var nameToAdd = records[i].playerName;
      if (i%2 === 0) {
        redPlayers.push(nameToAdd);
      } else {
        bluePlayers.push(nameToAdd);
      }
    }
    
    createdGameRecord.redPlayers = JSON.stringify(redPlayers);
    createdGameRecord.bluePlayers = JSON.stringify(bluePlayers);
    createdGameRecord.beginTime = getTime();
    
    schedule.add("update", "games", createdGameRecord, doNothing);
  });
});

function refreshLobbyAnd(callback) {
  readRecords("players", {}, function(records) {
    var players = "";
    for (var i=0; i<records.length; i++) {
      if (records[i].creatorName == myRecord.creatorName) {
        players += records[i].playerName + "\n";
      }
    }
    
    setText("lobbyPlayers", players);
    callback();
  });
}

function refreshGamesDropdownAnd(callback) {
  readRecords("games", {}, function(records) {
    var options = [];
    // lets run through the options in reverse order, so the most recent is displayed first
    for (var i=records.length-1; i>=0; i--) {
      options.push(records[i].creator);
    }
    
    setProperty("gamesDropdown", "options", options);
    callback();
  });
}

onEvent("joinGameButton", "click", function() {
  // attempt to join the game
  var name = getText("nameInput2");
  if (!isNameValid(name)) {
    setText("joinGameError", "Your name is invalid");
  } else {
    var gameToJoin = getText("gamesDropdown");
    readRecords("games", {creator:gameToJoin}, function(records) {
      if (records.length === 0) {
        setText("joinGameError", "There is no game by that name.");
      } else if (records.length > 1) {
        setText("joinGameError", "There are multiple games by that name.");
      } else {
        readRecords("players", {creatorName:gameToJoin}, function(records) {
          var any = false;
          for (var i=0; i<records.length; i++) {
            if (records[i].playerName == name) {
              // there is already somebody in that lobby by your name
              any = true;
              break;
            }
          }
          
          if (any) {
            setText("joinGameError", "There is already somebody in that game by your name");
          } else {
            var record = {
              playerName: name,
              creatorName: gameToJoin,
              data: "",
              sentTime: null,
            };
            
            schedule.add("create", "players", record, function(record) {
              myRecord = record;
              shouldRefreshDropdown = false;
              shouldRefreshLobby = true;
              listenForBegin = true;
              
              refreshLobbyAnd(function() {setScreen("lobbyScreen");});
            });
          }
        });
      }
    });
  }
});



onEvent("gotoLobbyButton", "click", function() {
  // we're gonna do all of the game setup stuff here
  // lets create the record with all of the game data stuff
  var name = getText("nameInput");
  if (!isNameValid(name)) {
    setText("createGameErrorLabel", "That name is invalid.");
  } else {
    // lets check through the game records to see if there is another game created
    // by the name 
    readRecords("games", {creator:name}, function(records) {
      // if we get any records back we cant start the game
      if (records.length !== 0) {
        setText("createGameErrorLabel", "There is already a game created by a player with your name.");
      } else {
        var record = {
          playerName: name,
          creatorName: name, // we're the creator, duh!
          data: "",
          sentTime: null,
        };
        
        schedule.add("create", "players", record, function(myNewRecord) {
          myRecord = myNewRecord;
          var record = {
            randomSeed: randomNumber(1,99999),
            gameMode: getProperty("gameModeDropdown", "index"),
            creator: name,
            beginTime: null,
          };
          
          schedule.add("create", "games", record, function(newRecord) {
            createdGameRecord = newRecord;
            shouldRefreshDropdown = false;
            shouldRefreshLobby = true;
            listenForBegin = true;
            showElement("beginButton");
            showElement("label9");
            
            refreshLobbyAnd(function() {setScreen("lobbyScreen");});
          });
        });
      }
    });
  }
});

onEvent("gotoCreateGame", "click", function() {
  setScreen("createGameScreen");
});
onEvent("back1", "click", function() { 
  setScreen("startScreen");
});

///////////////////////////////////////////////////////////////
//                   SPRITES
///////////////////////////////////////////////////////////////

function Sprites() {
  //  manipulating sprites and everything
  // includes both the grid and the sprite
  
  this.getSprite = function (id) {
    return this.spritesMap[id];
  };
  
  this.checkBackgroundCollisions = function () {
    throw "no";
  };


  this.checkSpriteCollisions = function () {
    throw "this seems kinda useless";
  };

  this.deleteSprite = function (id) {
    this.spritesMap[id].delete();
    delete this.spritesMap[id];
  };

  this.addSprite = function (constructor, x, y) {
    var id = this.getNewId();
    
    var sprite = new constructor(x, y, this.map, id);
    this.spritesMap[id] = sprite;
    return sprite;
  };
  
  this.addSpriteRaw = function (id, sprite) {
    this.spritesMap[id] = sprite;
  };
  
  this.resizeUpdate = function () {
    // redraws the sprites and the grid
    this.map = new Map(this.topLeftX, this.topLeftY, this.scaleFactor);

    for (var spriteIndex in this.spritesMap) {
      this.spritesMap[spriteIndex].setMap(this.map);
    }
  };
  
  this.getNewId = function () {
    // gets a new id for spawning in a ship
    var id = this.nextId.toString();
    this.nextId++;
    
    return id;
  };
  
  
  this.nextId = 0;
  
  this.spritesMap = {};

  this.map = new Map(0, 0, 1);

  var that = this;
  
  this.topLeftX = 0;
  this.topLeftY = 0;
  this.scaleFactor = 1;
  
  var background = new BackgroundSprite(160, 225, this.map, "background");
  this.spritesMap["background"] = background; // jshint ignore:line

  
  onEvent("background", "keydown", function(event) {
    if (event.key == "Up" | event.key == "Down") {
      var f = (event.key=="Up")? 1.1 : 1/1.1;
      
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

function Carrier(x, y, id, map, team) {
  var width = 120;
  var height = 20;
  var imageName = (team=="red")? "assets/redCarrier.png" : "assets/blueCarrier.png";
  var hitboxName = "carrier";
  var onHitByShip = function (other) {
    if (other.team == that.team) return; 
    
    that.health -= 100;
    if (that.health < 0) {
      client.sprites.deleteSprite(that.id);
      
      console.log("dead");
    }
  };
  
  BoatSprite.call(this, x, y, width, height, map, id, hitboxName, imageName, onHitByShip);
  
  var that = this;
  
  this.health = 500;
  this.team = team;
}

function BoatSprite(x, y, width, height, map, id, hitboxName, imageName, onHitByShip) {
  var hitbox = hitboxFrom(hitboxName, x, y);
  Sprite.call(this, x, y, width, height, map, id, hitbox, imageName);

  var that = this;
  
  this.onHitByShip = onHitByShip;

  this.backUp = function () {
    // will literally move the boat backwards
    var reverseAngle = (that.angle + TAU/2)%TAU; // we want the angle heading in the other direction
    var dx = Math.cos(reverseAngle);
    var dy = Math.sin(reverseAngle);
    that.moveToHelper(20, dx, dy); // lets do 20 pixels back
  };

  this.moveTo = function(pos) {
    var xDistance = pos[0] - that.x;
    var yDistance = pos[1] - that.y;
  
    that.rotateToAnd(Math.atan2(yDistance, xDistance), function(actualAngle) {
      // this is what we do after we rotate
      var distanceToMove = Math.sqrt(xDistance*xDistance + yDistance*yDistance);
      that.moveToHelper(distanceToMove, Math.cos(actualAngle), Math.sin(actualAngle));
    });
  };

  this.rotateToAnd = function (newAngle, callback) {
    var currentAngle = that.angle;
    var angleDistance = newAngle - currentAngle;
    // we want to move in the direction that requires the least movement
    var option1 = angleDistance%TAU; // going left
    var option2 = TAU-option1; // going right

    var change = (Math.abs(option1) < Math.abs(option2)) ? option1 : option2;
    var stepsLeft = Math.round(Math.abs(change) / 0.1); // 0.1 is our step size
    if (stepsLeft===0) return;// i don't want to deal with this crap in the loop

    var dTheta = 0.1*sign(change);

    that.rotateTimeoutInner(stepsLeft, currentAngle, dTheta, callback);
  };
  
  this.rotateTimeoutInner = function(stepsLeft, currentAngle, dTheta, doneCallback) {
    if (stepsLeft === 0) {
      // we are done
      doneCallback(currentAngle);
      return;
    }
    
    that.setAngle(currentAngle+dTheta);
    
    if (that.collidesWithBackground()) {
        // end the loop here too
      that.setAngle(currentAngle);
      return;
    }
    
    if (that.collideWithOtherSprites()) {
      return;
    }
    
    // if we didn't return above lets run this function again
    setTimeout(function() {
      that.rotateTimeoutInner(stepsLeft-1, currentAngle+dTheta, dTheta, doneCallback);
    }, 10);
  };
  
  this.moveToHelper = function(distanceToMove, dx, dy/*, actualAngle*/) {
    var stepSize = 3;
    
    dx *= stepSize;
    dy *= stepSize;
    
    var currentPosX = that.x;
    var currentPosY = that.y;
    
    var stepsLeft = Math.round(distanceToMove/stepSize); // because [dx, dy] is a unit vector
    if (stepsLeft === 0) return; // i want to keep the strict equality in the line after stepsLeft--
 
    that.moveHelperTimeoutInner(stepsLeft, currentPosX, currentPosY, dx, dy);
  };
  
  this.moveHelperTimeoutInner = function(stepsLeft, currentPosX, currentPosY, dx, dy) {
    if (stepsLeft === 0) return;
  
    that.setLocation(currentPosX+dx, currentPosY+dy);
    
    if (that.collidesWithBackground()) {
      that.setLocation(currentPosX, currentPosY); // move it backards if it crashed
      return;
    }

    // check if we collide with any other sprites
    if (that.collideWithOtherSprites()) {
      return;
    }

    // if we haven't returned yet, we know we need another iteration
    setTimeout(function() {
      that.moveHelperTimeoutInner(stepsLeft-1, currentPosX+dx, currentPosY+dy, dx, dy);
    }, 20);
  };
  
  this.collideWithOtherSprites = function() {
    for (var otherId in client.sprites.spritesMap) {
      var other = client.sprites.spritesMap[otherId];
      
      if (otherId == that.id || other.onHitByShip === undefined || !that.collidesWithOther(other)) continue;
      
      // if we got past htat continue we know the other ship was just hit
      other.onHitByShip(that);
      return true;
    }
    
    return false;
  };

  onEvent(this.id, "click", function (event) {
    // set up the toolbar 
    client.toolbar.setButtons([
      // move button
      {label:"move", image: "assets/moveIcon.png", callback: function() {
        var localPos = that.map.fromScreen(event.x, event.y);
        client.createPointer(localPos, MoveCrosshair, function(endPos) {
          client.requestBoatMove(that.id, endPos);
          //that.moveTo(endPos);
        });
      }},
      
      // back up button
      {label:"back up", image: "assets/backUpIcon.png", callback: function() {
        // back up a couple of steps
        client.requestBackUp(that.id);
      }},
      
      // fire button
      {label:"shoot", image: "assets/shootIcon.png", callback: function() {
        console.log("tried to shoot");
      }},
    ]);

  });
}

function MoveCrosshair(x, y, map, id) {
  var hitbox = hitboxFrom("none", x, y);
  Sprite.call(this, x, y, 45, 45, map, id, hitbox, "assets/moveIcon.png");
}

function SquareSprite(x, y, map, id) {
  var hitbox = hitboxFrom("square", x, y);
  
  Sprite.call(this, x, y, 28.28, 28.28, map, id, hitbox, "assets/square.png");
  
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
  
  onEvent(id, "click", function (pos) {
    client.toolbar.setButtons([
      {label: "add boat", image: "icon://fa-plus", callback: function () {
        
        client.createPointer(pos, MoveCrosshair, function (localPos) {
          client.sprites.addSprite(CarrierSprite, localPos[0], localPos[1]);
        });
      }},
      
      // open up the chat
      {label: "chat", image: "assets/openChat.png", callback: function () {
        hidePlayerLabels();
        showChat();
      }},
      
      {label: "show players", image: "icon://fa-gamepad", callback: function() {
        hideChat();

        showPlayerLabels();

      }},
    ]);
  });
}

var playerLabelsVisable = false;


function updatePlayerLabels() {
  if (!playerLabelsVisable) return; // lets not waste precios compute resources
  
  var activePlayerName = client.action.currentPlayersName();

  var string = "";
  var i, player;
  
  for (i=0; i<gameRecord.redPlayers.length; i++) {
    player = gameRecord.redPlayers[i];
    string += player;
    if (player == activePlayerName) string += "✓";
    string += "\n";
  }
  
  setText("redPlayersLabel", string);
  
  string = "";
  for (i=0; i<gameRecord.bluePlayers.length; i++) {
    player = gameRecord.bluePlayers[i];
    string += player;
    if (player == activePlayerName) string += "✓";
    string += "\n";
  }
  setText("bluePlayersLabel", string);
}

function showPlayerLabels() {


  showElement("redPlayersLabel");
  showElement("bluePlayersLabel");
  showElement("hidePlayerLabels");
    
  playerLabelsVisable = true;
    
  updatePlayerLabels();

}

function hidePlayerLabels() {
  hideElement("redPlayersLabel");
  hideElement("bluePlayersLabel");
  hideElement("hidePlayerLabels");
  
  playerLabelsVisable = false;
}


onEvent("hidePlayerLabels", "click", function() {
  hidePlayerLabels();
});


function Sprite(x, y, width, height, map, id, hitbox, imageName) {
  this.collidesWithBackground = function () {
    return this.hitbox.collidesWithBackground();
  };
  
  this.collidesWithOther = function(other) {
    return this.hitbox.collidesWithOther(other.hitbox);
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
      var thisLines = this.getSlopeInterceptLines();
      var otherLines = other.getSlopeInterceptLines();
      
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

    for (var i=0; i<this.vertices.length; i++) {
      var
        vertex0 = this.vertices[i],
        r0 = vertex0[0],
        newAngle0 = vertex0[1]+angle,
        x0 = this.x+r0*Math.cos(newAngle0),
        y0 = this.y+r0*Math.sin(newAngle0),
        
        vertex1 = this.vertices[(i+1<this.vertices.length) ? i+1 : 0],
        r1 = vertex1[0],
        newAngle1 = vertex1[1]+angle,
        x1 = this.x+r1*Math.cos(newAngle1),
        y1 = this.y+r1*Math.sin(newAngle1);
        
      /*
      
      var 
        start = this.getPoint(i),
        x0 = Math.round(start[0]),
        y0 = Math.round(start[1]),
        end = this.getPoint(i+1),
        x1 = Math.round(end[0]),
        y1 = Math.round(end[1]);
      */
      
      if (plotLine(x0, y0, x1, y1)) {
        return true;
      }
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




function plotLine(x0, y0, x1, y1) {
  // https://en.wikipedia.org/wiki/Bresenham%27s_line_algorithm
  
  x0 |= 0;
  y0 |= 0;
  x1 |= 0;
  y1 |= 0;
  
  if (Math.abs(y1 - y0) < Math.abs(x1 - x0)) {
    if (x0 > x1) {
      return plotLineLow(x1, y1, x0, y0);
    } else {
      return plotLineLow(x0, y0, x1, y1);
    }
      
  } else {
    if (y0 > y1) {
      return plotLineHigh(x1, y1, x0, y0);
    } else {
      return plotLineHigh(x0, y0, x1, y1);
    }
      
  }
}

function plotLineHigh(x0, y0, x1, y1) {
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
    var i = x + y*320;
    if ((MAPS[backgroundIndex][(i/32)|0] >> i%32) & 1) {
      return true;
    }
    if (D > 0) {
      x += xi;
      D -= 2*dy;
    }
    D += 2*dx;
  }
  
  return false;
}

// taken from wikipedia bresenham's article
function plotLineLow(x0, y0, x1, y1) {
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
    var i = x + y*320;
    if ((MAPS[backgroundIndex][(i/32)|0] >> i%32) & 1) {
      return true;
    }
    if (D > 0) {
      y += yi;
      D -= 2*dx;
    }
    D += 2*dy;
  }
  
  return false;
}


function isSolid(x, y) {
  var i = x + y*320;
  return (MAPS[backgroundIndex][(i/32)|0] >> i%32) & 1;
/*
  var i = x + y*320;
  var numberIndex = (i/32)|0;
  var numberShift = i % 32;
  
  var number = MAPS[backgroundIndex][numberIndex];
  
  return (number >> numberShift) & 1;
*/
}





function Toolbar() {
  this.buttons = [];
  
  this.deleteButtons = function () {
    for (var i=0; i<this.buttons.length; i++) {
      this.buttons[i].delete();
      this.buttons[i] = null;
    }
    
    this.buttons = [];
  };
  
  this.setButtons = function (newButtons) {
    this.deleteButtons();

    var y = 330;
    for (var i=0; i<newButtons.length; i++) {
      var x = 80*i;

      this.buttons.push(
        new ToolbarButton(x, y, newButtons[i].label, newButtons[i].image, newButtons[i].callback));
    }
  };
  
  this.setLabel = function (newLabel) {
    setText("toolbarLabel", newLabel);
  };
  
  this.hide = function () {
    hideElement("toolbarLabel");
    
    for (var i=0; i<this.buttons.length; i++) {
      this.buttons[i].hide();
    }
  };
  
  this.show = function () {
    showElement("toolbarLabel");
    
    for (var i=0; i<this.buttons.length; i++) {
      this.buttons[i].show();
    }
  };
}



function ToolbarButton(x, y, label, image, clickCallback) {
  this.id = uniqueId();
  button(this.id, label);
  setProperty(this.id, "background-color", rgb(0.6, 0.6, 0.6, 0.5));
  setProperty(this.id, "text-color", "gold");

  setStyle(this.id, "font-weight: bolder;");

  setPosition(this.id, x, y, 80, 80);
  setProperty(this.id, "image", image);
  setStyle(this.id, "padding: 0px;");
  setStyle(this.id, "border: 2px solid #000000;");

 
  onEvent(this.id, "click", clickCallback);
  
  this.delete = function () {
    deleteElement(this.id);
  };
  
  this.hide = function () {
    hideElement(this.id);
  };
  
  this.show = function () {
    showElement(this.id);
  };
}






////////////////////////////////////////////////
//// CHAT STUFF
////////////////////////////////////////////////

var chatIsShown = false;

onEvent("chatInput", "keydown", function(event) {
  if (event.key == "Enter") {
    var text = getText("chatInput");
    setText("chatInput", "");

    client.sendMessage(text);
  }
});



function showChat() {
  for (var i=0; i<chatElements.length; i++) {
    showElement(chatElements[i]);
  }
  
  chatIsShown = true;
}

onEvent("closeChatButton", "click", hideChat);

function hideChat() {
  for (var i=0; i<chatElements.length; i++) {
    hideElement(chatElements[i]);
  }
  
  chatIsShown = false;
}

function aslInterpret(text) {
  showElement("aslOutput");
  setSize("output", 250, 80);
  var index = 0;
  
  var accurateAslCallback = function () {
    // choose the sign
    var signIndex = text.charCodeAt(index) % signs.length;
    setImageURL("aslOutput", signs[signIndex]);
    
    index++;
    if (index >= text.length) {
      hideElement("aslOutput");
      setSize("chatOutput", 320, 80);
      return;
    }
    
    var delay = randomNumber(50, 150);
    setTimeout(accurateAslCallback, delay);
  };
  
  accurateAslCallback();
}


///////////////////////////////////////////////////////////////
//                    SMALL HELPER FUNCTIONS
///////////////////////////////////////////////////////////////




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
    if (this.buf.length === 0 || this.lastUpdated+200 > getTime()) return;

    this.buf.pop()(); // call the function
    this.lastUpdated = getTime();
  };
  
  var that = this;
  timedLoop(100, function() {
    that.update();
  });
}


function isNameValid(name) {
  // name connot be blank, and must contain at least
  // one non-whitespace character
  return name.length > 0 && name.length < 20;
}

function map(x, in_min, in_max, out_min, out_max) {
  // https://www.arduino.cc/reference/en/language/functions/math/map/
  return (x - in_min) * (out_max - out_min) / (in_max - in_min) + out_min;
}


function doNothing() {}


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


function randomColor() {
  return rgb(
    randomNumber(0,255),
    randomNumber(0,255),
    randomNumber(0,255)
  );
}



////////////////////////////////////////////////////////////////
//                       DATA
////////////////////////////////////////////////////////////////

var chatElements = [
  "chatInput",
  "chatOutput",
  "aslLabel",
  "aslCheckbox",
  "closeChatButton",
  "aslOutput",
  
  // chese aren't chat elements
];

setStyle("whosTurn", "z-index: 999;");
setStyle("timeLeft", "z-index: 999;");
setStyle("nActionPoints", "z-index: 999;");
setStyle("skipTurnButton", "z-index: 999;");

setStyle("redPlayersLabel", "z-index: 999;");
setStyle("bluePlayersLabel", "z-index: 999;");
setStyle("hidePlayerLabels", "z-index: 999;");

setStyle("actionPointsLabel", "z-index: 999;");

for (var i=0; i<chatElements.length; i++) {
  setStyle(chatElements[i], "z-index: 999;");
}

var signs = [
  // these are actual asl signs look it up its pretty cool
  "icon://fa-hand-rock-o",
  "icon://fa-hand-paper-o",
  "icon://fa-hand-scissors-o",
  "icon://fa-hand-spock-o",
  "icon://fa-hand-lizard-o",
  
  "icon://fa-hand-o-down",
  "icon://fa-hand-o-up",
  "icon://fa-hand-o-right",
  "icon://fa-hand-o-left",
  "icon://fa-thumbs-o-down",
  "icon://fa-thumbs-o-up",
  
  // facial expressions are very important for asl
  "icon://fa-smile-o",
  "icon://fa-frown-o",
  "icon://fa-meh-o",

  "icon://fa-angellist",
  "icon://fa-tripadvisor",
];


var MAPS = [
  [0,0,3758096384,4294967295,511,0,0,2147483648,4294967295,524287,0,0,3758096384,4294967295,1023,0,0,0,4294967295,524287,0,0,4026531840,4294967295,1023,0,0,0,4294967295,1048575,0,0,4026531840,4294967295,2047,0,0,0,4294967294,1048575,0,0,4026531840,4294967295,4095,0,0,0,4294967294,1048575,0,0,4026531840,4294967295,8191,0,0,0,4294967292,2097151,0,0,4026531840,4294967295,16383,0,0,0,4294967292,2097151,2031616,0,4026531840,4294967295,32767,0,0,0,4294967292,4194303,8372224,0,4160749568,4294967295,32767,0,0,0,4294967288,4194303,16769024,0,4160749568,4294967295,65535,0,0,0,4294967288,4194303,33550336,0,4160749568,4294967295,131071,0,0,0,4294967280,8388607,67106816,0,4160749568,4294967295,262143,0,0,0,4294967280,8388607,134216704,0,4160749568,4294967295,524287,0,0,0,4294967280,8388607,268434432,0,4227858432,4294967295,2097151,0,0,0,4294967264,16777215,268434944,0,4227858432,4294967295,4194303,0,0,0,4294967264,16777215,536870656,0,4227858432,4294967295,8388607,0,0,0,4294967264,16777215,1073741696,0,4227858432,4294967295,16777215,0,0,0,4294967232,16777215,2147483584,0,4261412864,4294967295,67108863,0,0,0,4294967232,16777215,4294967232,0,4261412864,4294967295,134217727,0,0,0,4294967232,33554431,4294967264,0,4261412864,4294967295,536870911,0,0,0,4294967168,33554431,4294967280,1,4278190080,4294967295,2147483647,0,0,0,4294967168,33554431,4294967280,3,4278190080,4294967295,4294967295,1,0,0,4294967168,33554431,4294967288,7,4278190080,4294967295,4294967295,7,0,0,4294967040,33554431,4294967292,7,4286578688,4294967295,4294967295,15,0,0,4294967040,33554431,4294967294,15,4286578688,4294967295,4294967295,31,0,0,4294966784,16777215,4294967295,31,4286578688,4294967295,4294967295,63,0,0,4294966784,16777215,4294967295,63,4290772992,4294967295,4294967295,127,0,0,4294966784,16777215,4294967295,127,4290772992,4294967295,4294967295,127,0,0,4294966272,16777215,4294967295,127,4292870144,4294967295,4294967295,127,0,0,4294966272,8388607,4294967295,255,4292870144,4294967295,4294967295,255,0,0,4294965248,8388607,4294967295,511,4293918720,4294967295,4294967295,255,0,0,4294965248,8388607,4294967295,1023,4293918720,4294967295,4294967295,255,0,0,4294963200,4194303,4294967295,2047,4294443008,4294967295,4294967295,255,0,0,4294963200,2097151,4294967295,4095,4294443008,4294967295,4294967295,255,0,0,4294959104,2097151,4294967295,8191,4294705152,4294967295,4294967295,255,0,0,4294950912,1048575,4294967295,16383,4294705152,4294967295,4294967295,255,0,0,4294950912,524287,4294967295,32767,4294836224,4294967295,4294967295,127,0,0,4294934528,262143,4294967295,65535,4294901760,4294967295,4294967295,127,0,0,4294901760,131071,4294967295,131071,4294934528,4294967295,4294967295,63,0,0,4294836224,65535,4294967295,524287,4294934528,4294967295,4294967295,63,0,0,4294705152,16383,4294967295,1048575,4294950912,4294967295,4294967295,31,0,0,4293918720,8191,4294967295,2097151,4294959104,4294967295,4294967295,31,0,0,4292870144,2047,4294967295,8388607,4294963200,4294967295,4294967295,15,0,0,4278190080,255,4294967295,33554431,4294966272,4294967295,4294967295,7,0,0,4026531840,7,4294967295,134217727,4294966784,4294967295,4294967295,3,0,0,0,0,4294967295,1073741823,4294967168,4294967295,4294967295,1,0,0,0,0,4294967295,4294967295,4294967295,4294967295,4294967295,1,0,0,0,0,4294967295,4294967295,4294967295,4294967295,4294967295,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,2147483647,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,1073741823,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,536870911,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,268435455,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,134217727,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,67108863,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,67108863,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,33554431,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,16777215,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,8388607,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,8388607,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,4194303,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,2097151,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,1048575,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,1048575,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,524287,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,262143,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,131071,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,131071,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,65535,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,32767,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,16383,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,16383,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,8191,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,4095,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,2047,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,1023,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,511,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,255,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,127,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,63,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,31,0,0,0,0,0,4294967295,4294967295,4294967295,4294967295,7,0,0,0,0,0,4294967295,4294967295,2147483647,4294901760,0,0,2147483648,15,0,0,4294967295,4294967295,268435455,0,0,0,3221225472,63,0,0,4294967295,4294967295,67108863,0,0,0,3758096384,63,0,0,4294967295,4294967295,16777215,0,0,0,4026531840,127,0,0,4294967295,4294967295,8388607,0,0,0,4160749568,255,0,0,4294967295,4294967295,4194303,0,0,0,4227858432,255,0,0,4294967295,4294967295,1048575,0,0,0,4227858432,255,0,0,4294967295,4294967295,524287,0,0,0,4227858432,255,0,0,4294967295,4294967295,262143,0,0,0,4261412864,255,0,0,4294967295,4294967295,131071,0,0,0,4261412864,511,0,0,4294967295,4294967295,65535,0,0,0,4261412864,255,0,0,4294967295,4294967295,32767,0,0,0,4261412864,255,0,0,4294967295,4294967295,16383,0,0,0,4261412864,255,0,0,4294967295,4294967295,16383,0,0,0,4261412864,255,0,0,4294967295,4294967295,8191,0,0,0,4261412864,255,0,0,4294967295,4294967295,4095,0,0,0,4261412864,127,0,0,4294967280,4294967295,2047,0,0,0,4227858432,127,0,0,4294967168,4294967295,1023,0,0,0,4227858432,63,0,0,4294966272,4294967295,511,0,0,0,4227858432,63,0,0,4294963200,4294967295,255,0,0,0,4160749568,31,0,0,4294950912,4294967295,255,0,0,0,4026531840,15,0,0,4294901760,4294967295,127,0,0,0,3758096384,7,0,0,4294836224,4294967295,63,0,0,0,0,0,0,0,4294443008,4294967295,31,0,0,0,0,0,0,0,4293918720,4294967295,15,0,0,0,0,0,0,0,4292870144,4294967295,7,0,0,0,0,0,0,0,4286578688,4294967295,3,0,0,0,0,0,0,0,4278190080,4294967295,3,0,0,0,0,0,0,0,4261412864,4294967295,1,0,0,0,0,0,0,0,4227858432,4294967295,0,0,0,0,0,0,0,0,4026531840,2147483647,0,0,0,0,0,0,0,0,3758096384,1073741823,0,0,0,0,0,0,0,0,3221225472,536870911,0,0,0,0,0,0,0,0,0,134217727,0,0,0,0,0,0,0,0,0,67108862,0,0,0,0,0,0,0,0,0,33554424,0,0,0,0,0,0,0,0,0,8388592,0,0,0,0,0,0,0,0,0,2097024,0,0,65024,0,0,0,0,0,0,523264,0,0,130816,0,0,0,0,0,0,0,0,0,524160,0,0,0,0,0,0,0,0,0,1048512,0,0,0,0,0,0,0,0,0,2097120,0,0,0,0,0,0,0,0,0,4194272,0,0,0,0,0,0,0,0,0,8388592,0,0,0,0,0,0,0,0,0,16777200,0,0,0,0,0,0,0,0,0,33554424,0,0,0,0,0,0,0,0,0,67108856,0,0,0,0,0,0,0,0,0,134217724,0,0,0,0,0,0,0,0,0,268435452,0,0,0,0,0,0,0,0,0,536870908,0,0,0,0,2147483648,0,0,0,0,1073741822,0,0,0,0,3758096384,0,0,0,0,2147483646,0,0,0,0,4160749568,0,0,0,0,2147483647,0,0,0,0,4227858432,0,0,0,0,4294967295,0,0,0,0,4278190080,0,0,0,0,4294967295,1,0,0,0,4286578688,0,0,0,2147483648,4294967295,3,0,0,0,4290772992,0,0,0,2147483648,4294967295,7,0,0,0,4292870144,0,0,0,2147483648,4294967295,15,0,0,0,4293918720,0,0,0,3221225472,4294967295,31,0,0,0,4294705152,0,0,0,3221225472,4294967295,63,0,0,0,4294705152,0,0,0,3221225472,4294967295,127,0,0,0,4294836224,0,0,0,3221225472,4294967295,255,0,0,0,4294901760,0,0,0,3758096384,4294967295,1023,0,0,0,4294934528,0,0,0,3758096384,4294967295,2047,0,0,0,4294950912,0,0,0,3758096384,4294967295,4095,0,0,0,4294959104,0,0,0,4026531840,4294967295,8191,0,0,0,4294959104,0,0,0,4026531840,4294967295,16383,0,0,0,4294963200,0,0,0,4026531840,4294967295,32767,0,0,0,4294965248,0,0,0,4026531840,4294967295,65535,0,0,0,4294965248,0,0,0,4160749568,4294967295,65535,0,0,0,4294966272,0,0,0,4160749568,4294967295,131071,0,0,0,4294966784,0,0,0,4160749568,4294967295,262143,0,0,0,4294966784,0,0,0,4160749568,4294967295,524287,0,0,0,4294967040,0,0,0,4227858432,4294967295,524287,0,0,0,4294967040,0,0,0,4227858432,4294967295,1048575,0,0,0,4294967168,0,0,0,4227858432,4294967295,1048575,0,0,0,4294967168,0,0,0,4227858432,4294967295,2097151,0,0,0,4294967232,0,0,0,4261412864,4294967295,2097151,0,0,0,4294967232,0,0,0,4261412864,4294967295,4194303,0,0,0,4294967264,0,0,0,4261412864,4294967295,4194303,0,0,0,4294967264,0,0,0,4261412864,4294967295,4194303,0,0,0,4294967280,0,0,0,4278190080,4294967295,8388607,0,0,0,4294967280,0,0,0,4278190080,4294967295,8388607,0,0,0,4294967288,0,0,0,4278190080,4294967295,8388607,0,0,0,4294967288,0,0,0,4286578688,4294967295,8388607,0,0,0,4294967288,0,0,0,4286578688,4294967295,8388607,0,0,0,4294967292,0,0,0,4286578688,4294967295,8388607,0,0,0,4294967292,0,0,0,4286578688,4294967295,8388607,0,0,0,4294967294,0,0,0,4290772992,4294967295,16777215,0,0,0,4294967294,0,0,0,4290772992,4294967295,16777215,0,0,0,4294967294,0,0,0,4290772992,4294967295,16777215,0,0,0,4294967295,0,0,0,4290772992,4294967295,16777215,0,0,0,4294967295,0,0,0,4292870144,4294967295,16777215,0,0,0,4294967295,0,0,0,4292870144,4294967295,8388607,0,0,2147483648,4294967295,0,0,0,4292870144,4294967295,8388607,0,0,2147483648,4294967295,0,0,0,4293918720,4294967295,8388607,0,0,2147483648,4294967295,0,0,0,4293918720,4294967295,8388607,0,0,3221225472,4294967295,0,0,0,4293918720,4294967295,8388607,0,0,3221225472,4294967295,0,0,0,4293918720,4294967295,8388607,0,0,3221225472,4294967295,0,0,0,4294443008,4294967295,8388607,0,0,3758096384,4294967295,0,0,0,4294443008,4294967295,4194303,0,0,3758096384,4294967295,0,0,0,4294705152,4294967295,4194303,0,0,3758096384,4294967295,0,0,0,4294705152,4294967295,4194303,0,0,3758096384,4294967295,0,0,0,4294705152,4294967295,4194303,0,0,4026531840,4294967295,0,0,0,4294836224,4294967295,2097151,0,0,4026531840,4294967295,0,0,0,4294836224,4294967295,2097151,0,0,4026531840,4294967295,0,0,0,4294836224,4294967295,2097151,0,0,4160749568,4294967295,0,0,0,4294901760,4294967295,1048575,0,0,4160749568,4294967295,0,0,0,4294901760,4294967295,1048575,0,0,4160749568,4294967295,0,0,0,4294934528,4294967295,1048575,0,0,4227858432,4294967295,0,0,0,4294934528,4294967295,524287,0,0,4227858432,4294967295,0,0,0,4294934528,4294967295,524287,0,0,4227858432,4294967295,0,0,0,4294950912,4294967295,262143,0,0,4227858432,4294967295,0,0,0,4294950912,4294967295,262143,0,0,4261412864,4294967295,0,0,0,4294959104,4294967295,131071,0,0,4261412864,4294967295,0,0,0,4294959104,4294967295,65535,0,0,4261412864,4294967295,0,0,0,4294963200,4294967295,65535,0,0,4278190080,4294967295,0,0,0,4294963200,4294967295,32767,0,0,4278190080,4294967295,0,0,0,4294965248,4294967295,16383,0,0,4278190080,4294967295,0,0,0,4294965248,4294967295,8191,0,0,4286578688,4294967295,0,0,0,4294966272,4294967295,4095,0,0,4286578688,4294967295,0,0,0,4294966272,4294967295,2047,0,0,4290772992,4294967295,0,0,0,4294966784,4294967295,1023,0,0,4290772992,4294967295,0,0,0,4294966784,4294967295,511,0,0,4290772992,4294967295,0,0,0,4294967040,4294967295,255,0,0,4292870144,4294967295,0,0,0,4294967168,4294967295,63,0,0,4292870144,4294967295,0,0,0,4294967168,4294967295,15,0,0,4293918720,4294967295,0,0,0,4294967232,4294967295,3,0,0,4294443008,4294967295,0,0,0,4294967232,2147483647,0,0,0,4294443008,4294967295,0,0,0,4294967264,536870911,0,0,0,4294705152,1048575,0,0,0,4294967264,134217727,0,0,0,4294705152,32767,0,0,0,4294967280,33554431,0,0,0,4294836224,2047,0,0,0,4294967280,8388607,0,0,0,4294901760,511,0,0,0,4294967288,2097151,0,0,0,4294934528,63,0,0,0,4294967288,1048575,0,0,0,4294950912,15,0,0,0,4294967292,524287,0,0,0,4294959104,7,0,0,0,4294967292,262143,0,0,0,4294959104,3,0,0,0,4294967292,65535,0,0,0,4294963200,0,0,0,0,4294967294,32767,0,0,0,2147481600,0,0,0,0,4294967294,32767,0,0,0,1073740800,0,0,0,0,4294967295,16383,0,0,0,1073741312,0,0,0,0,4294967295,8191,0,0,0,536870656,0,0,0,2147483648,4294967295,4095,0,0,0,268435328,0,0,0,2147483648,4294967295,2047,0,0,0,268435392,0,0,0,2147483648,4294967295,2047,0,0,0,134217696,0,0,0,3221225472,4294967295,1023,0,0,0,134217712,0,0,0,3221225472,4294967295,511,0,0,0,134217720,0,0,0,3758096384,4294967295,511,0,0,0,67108860,0,0,0,3758096384,4294967295,255,0,0,0,67108862,0,0,0,4026531840,4294967295,127,0,0,0,67108863,0,0,0,4026531840,4294967295,127,0,0,2147483648,33554431,0,0,0,4026531840,4294967295,63,0,0,3221225472,33554431,0,0,0,4160749568,4294967295,63,0,0,3758096384,33554431,0,0,0,4160749568,4294967295,31,0,0,3758096384,33554431,0,0,0,4227858432,4294967295,31,0,0,4026531840,16777215,0,0,0,4227858432,4294967295,31,0,0,4160749568,16777215,0,0,0,4261412864,4294967295,15,0,0,4160749568,16777215,0,0,0,4261412864,4294967295,15,0,0,4227858432,16777215,0,0,0,4261412864,4294967295,7,0,0,4261412864,16777215,0,0,0,4278190080,4294967295,7,0,0,4261412864,16777215,0,0,0,4278190080,4294967295,7,0,0,4261412864,8388607,0,0,0,4286578688,4294967295,3,0,0,4278190080,8388607,0,0,0,4286578688,4294967295,3,0,0,4278190080,8388607,0,0,0,4286578688,4294967295,3,0,0,4286578688,8388607,0,0,0,4290772992,4294967295,3,0,0,4286578688,4194303,0,0,0,4290772992,4294967295,1,0,0,4286578688,4194303,0,0,0,4290772992,4294967295,1,0,0,4286578688,4194303,0,1,0,4292870144,4294967295,1,0,0,4286578688,4194303,0,1,0,4292870144,4294967295,1,0,0,4286578688,2097151,0,3,0,4292870144,4294967295,1,0,0,4286578688,2097151,0,7,0,4293918720,4294967295,0,0,0,4286578688,2097151,0,7,0,4293918720,4294967295,0,0,0,4286578688,2097151,0,15,0,4293918720,4294967295,0,0,0,4286578688,1048575,0,15,0,4293918720,4294967295,0,0,0,4286578688,1048575,0,31,0,4294443008,4294967295,0,0,0,4278190080,524287,0,31,0,4294443008,4294967295,0,0,0,4278190080,524287,0,31,0,4294443008,2147483647,0,0,0,4261412864,262143,0,63,0,4294443008,2147483647,0,0,0,4261412864,131071,0,63,0,4294443008,2147483647,0,0,0,4227858432,131071,0,63,0,4294443008,2147483647,0,0,0,4160749568,65535,0,63,0,4294443008,2147483647,0,0,0,4026531840,16383,0,127,0,4294443008,2147483647,0,0,0,3221225472,8191,0,127,0,4294443008,2147483647,0,0,0,0,2046,0,127,0,4294705152,2147483647,0,0,0,0,0,0,127,0,4294705152,2147483647,0,0,0,0,0,0,127,0,4294705152,1073741823,0,0,0,0,0,0,127,0,4294705152,1073741823,0,0,0,0,0,0,127,0,4294443008,1073741823,0,0,0,0,0,0,127,0,4294443008,1073741823,0,0,0,0,0,0,127,0,4294443008,1073741823,0,0,0,0,0,0,127,0,4294443008,1073741823,0,0,0,0,0,0,127,0,4294443008,1073741823,0,0,0,0,0,0,127,0,4294443008,1073741823,0,0,0,0,0,0,63,0,4294443008,1073741823,0,0,0,0,0,0,63,0,4294443008,1073741823,0,0,0,0,0,0,63,0,4294443008,1073741823,0,0,0,0,0,0,63,0,4293918720,1073741823,0,0,0,0,0,0,63,0,4293918720,1073741823,0,0,0,0,0,0,31,0,4293918720,1073741823,0,0,0,0,0,0,31,0,4293918720,536870911,0,0,0,0,0,0,31,0,4293918720,536870911,0,0,0,0,0,0,15,0,4292870144,536870911,0,0,0,0,0,0,15,0,4292870144,536870911,0,0,0,0,0,0,15,0,4292870144,536870911,0,0,0,0,0,0,7,0,4292870144,536870911,0,0,0,0,0,0,7,0,4290772992,536870911,0,0,0,0,0,0,3,0,4290772992,536870911,0,0,0,0,0,0,3,0,4290772992,536870911,0,0,0,0,0,0,1,0,4286578688,536870911,0,0,0,0,0,0,1,0,4286578688,268435455,0,0,0,0,0,0,1,0,4286578688,268435455,0,0,0,0,0,0,0,0,4278190080,268435455,0,0,0,0,0,0,0,0,4278190080,268435455,0,0,0,0,0,0,0,0,4278190080,268435455,0,3758096384,63,0,0,0,0,0,4261412864,268435455,0,4261412864,511,0,0,0,0,0,4261412864,268435455,0,4290772992,4095,0,0,0,0,0,4261412864,134217727,0,4293918720,8191,0,0,0,0,0,4227858432,134217727,0,4294705152,16383,0,0,0,0,0,4227858432,134217727,0,4294836224,65535,0,0,0,0,0,4227858432,134217727,0,4294934528,65535,0,0,0,0,0,4160749568,67108863,0,4294950912,131071,0,0,0,0,0,4160749568,67108863,0,4294959104,262143,0,0,0,0,0,4160749568,67108863,0,4294963200,524287,0,0,0,0,0,4026531840,67108863,0,4294963200,524287,0,0,0,0,0,4026531840,33554431,0,4294965248,1048575,0,0,0,0,0,4026531840,33554431,0,4294966272,1048575,0,0,0,0,0,3758096384,33554431,0,4294966272,2097151,0,0,62914560,0,0,3758096384,16777215,0,4294966784,2097151,0,0,266338304,0,0,3758096384,16777215,0,4294966784,4194303,0,0,267386880,0,0,3221225472,16777215,0,4294967040,4194303,0,0,535822336,0,0,3221225472,8388607,0,4294967040,8388607,0,0,535822336,0,0,3221225472,8388607,0,4294967040,8388607,0,0,535822336,0,0,3221225472,8388607,0,4294967040,8388607,0,0,267386880,0,0,2147483648,4194303,0,4294967040,16777215,0,0,266338304,0,0,2147483648,4194303,0,4294967040,16777215,0,0,62914560,0,0,2147483648,2097151,0,4294967040,16777215,0,0,0,0,0,2147483648,2097151,0,4294967040,33554431,0,0,0,0,0,0,1048575,0,4294967040,33554431,0,0,0,0,0,0,1048575,0,4294967040,33554431,0,0,0,0,0,0,1048575,0,4294967040,67108863,0,0,0,0,0,0,524287,0,4294967040,67108863,0,0,0,0,0,0,524287,0,4294967040,67108863,0,0,0,0,0,0,262143,0,4294966784,134217727,0,0,0,0,0,0,262142,0,4294966784,134217727,0,0,0,0,0,0,262142,0,4294966784,134217727,0,0,0,0,0,0,131070,0,4294966272,268435455,0,0,0,0,0,0,131070,0,4294966272,268435455,0,0,0,0,0,0,131070,0,4294965248,536870911,0,0,0,0,0,0,65534,0,4294965248,536870911,0,0,0,0,0,0,65534,0,4294963200,536870911,0,0,0,0,0,0,65535,0,4294963200,1073741823,0,0,0,0,0,0,65535,0,4294959104,1073741823,0,0,0,0,0,0,32767,0,4294959104,2147483647,0,0,0,0,0,0,32767,0,4294950912,2147483647,0,0,0,0,0,0,32767,0,4294950912,4294967295,0,0,0,0,0,0,32767,0,4294934528,4294967295,0,0,0,0,0,0,32767,0,4294934528,4294967295,1,0,0,0,0,0,16383,0,4294901760,4294967295,1,0,0,0,0,2147483648,16383,0,4294901760,4294967295,3,0,0,0,0,2147483648,16383,0,4294836224,4294967295,3,0,0,0,0,2147483648,16383,0,4294836224,4294967295,7,0,0,0,0,2147483648,16383,0,4294705152,4294967295,15,0,0,0,0,2147483648,8191,0,4294705152,4294967295,15,0,0,0,0,2147483648,8191,0,4294443008,4294967295,31,0,0,0,0,2147483648,8191,0,4294443008,4294967295,31,0,0,0,0,2147483648,8191,0,4293918720,4294967295,63,0,0,0,0,2147483648,4095,0,4293918720,4294967295,127,0,0,0,0,2147483648,4095,0,4292870144,4294967295,127,0,0,0,0,0,2047,0,4292870144,4294967295,255,0,0,0,0,0,2047,0,4290772992,4294967295,511,0,0,0,0,0,1023,0,4290772992,4294967295,511,0,0,0,0,0,510,0,4286578688,4294967295,1023,0,0,0,0,0,252,0,4286578688,4294967295,2047,0,0,0,0,0,48,0,4278190080,4294967295,4095,0,0,0,0,0,0,0,4278190080,4294967295,4095,0,0,0,0,0,0,0,4261412864,4294967295,8191,0,0,0,0,0,0,0,4261412864,4294967295,16383,0,0,0,0,0,0,0,4227858432,4294967295,16383,0,0,0,0,0,0,0,4227858432,4294967295,32767,0,0,0,0,0,0,0,4160749568,4294967295,65535,0,0,0,0,0,0,0,4160749568,4294967295,65535,0,0,0,0,0,0,0,4160749568,4294967295,131071,0,0,0,0,0,0,0,4026531840,4294967295,262143,0,0,0,0,0,0,0,4026531840,4294967295,262143,0,0,0,0,0,0,0,3758096384,4294967295,524287,0,0,0,0,0,0,0,3758096384,4294967295,524287,0,0,0,0,0,0,0,3221225472,4294967295,1048575,0,0,0,0,0,0,0,3221225472,4294967295,1048575,0,0,0,0,0,0,0,3221225472,4294967295,2097151,0,0,0,0,0,0,0,2147483648,4294967295,2097151,0,0,0,0,0,0,0,2147483648,4294967295,2097151,0,0,0,0,0,0,0,2147483648,4294967295,4194303,0,0,0,0,0,0,0,0,4294967295,4194303,0,0,0,0,0,0,0,0,4294967295,4194303,0,0,0,0,0,0,0,0,4294967295,4194303,0,0,0,0,0,0,0,0,4294967294,8388607,0,0,0,0,0,0,0,0,4294967294,8388607,0,0,0,0,0,0,0,0,4294967294,8388607,0,0,0,0,0,0,0,0,4294967294,8388607,0,0,0,0,0,0,0,0,4294967292,8388607,0,0,0,0,0,0,0,0,4294967292,8388607,0,0,0,0,0,0,0,0,4294967292,8388607,0,0,0,0,0,0,0,0,4294967292,8388607,0,0,0,0,0,0,0,0,4294967288,8388607,0,0,0,0,0,0,0,0,4294967288,8388607,0,0,0,0,0,0,0,0,4294967288,8388607,0,0,0,0,0,0,0,0,4294967288,8388607,0,0,0,0,0,0,0,0,4294967288,4194303,0,0,0,0,0,0,0,0,4294967280,4194303,0,0,0,0,0,0,0,0,4294967280,4194303,0,0,0,0,0,0,0,0,4294967280,4194303,0,0,0,0,0,0,0,0,4294967280,4194303,0,0,0,0,0,0,0,0,4294967280,2097151,0,0,0,0,0,0,0,0,4294967280,2097151,0,0,0,0,0,0,0,0,4294967280,2097151,0,0,0,0,0,0,0,0,4294967280,1048575,0,0,0,0,0,0,0,0,4294967280,1048575,0,0,0,0,0,0,0,0,4294967280,1048575,0,0,0,0,0,0,0,0,4294967264,524287,0,0,0,0,0,0,0,0,4294967264,524287,0,0,0,0,0,0,0,0,4294967264,524287,0,0,0,0,0,0,0,0,4294967264,262143,0,0,0,0,0,0,0,0,4294967264,262143,0,0,0,0,0,0,0,0,4294967264,131071,0,0,0,0,0,0,0,0,4294967264,131071,0,0,0,0,0,0,0,0,4294967264,65535,0,0,0,0,0,0,0,0,4294967280,65535,0,0,0,0,0,0,0,0,4294967280,65535,0,0,0,0,0,0,0,0,4294967280,32767,0,0,0,0,0,0,0,0,4294967280,32767,0,0,0,0,0,0,0,0,4294967280,16383,0,0,0,0,0,0,0,0,4294967280,16383,0,0,0,0,0,0,0,0,4294967280,8191,0,0,0,0,0,0,0,0,4294967280,8191,0,0,0,0,0,0,0,0,4294967280,4095,0,0,0,0,0,0,0,0,4294967288,4095,0,0,0,0,0,0,0,0,4294967288,4095,0,0,0,0,0,0,0,0,4294967288,2047,0,0,0,0,0,0,0,0,4294967288,2047,0,0,0,0,0,0,0,0,4294967288,1023,0,0,0,0,0,0,0,0,4294967292,1023,0,0,0,0,0,0,0,0,4294967292,1023,0,0,0,0,0,0,0,0,4294967292,511,0,0,0,0,0,0,0,0,4294967294,511,0,0,0,0,0,0,0,0,4294967294,255,0,0,0,0,0,0,0,0,4294967294,255,0,0,0,0,0,0,0,0,4294967295,255,0,0,0,0,0,0,0,0,4294967295,255,0,0,0,0,0,0,0,0,4294967295,127,0,0,0,0,0,0,0,2147483648,4294967295,127,0,0,0,0,0,0,0,2147483648,4294967295,127,0,0,0,0,0,0,0,3221225472,4294967295,63,0,0,1056964608,0,0,0,0,3221225472,4294967295,63,0,0,4290772992,1,0,0,0,3758096384,4294967295,63,0,0,4292870144,3,0,0,0,3758096384,4294967295,63,0,0,4293918720,15,0,0,0,4026531840,4294967295,63,0,0,4294443008,31,0,0,0,4026531840,4294967295,63,0,0,4294443008,63,0,0,0,4160749568,4294967295,31,0,0,4294705152,127,0,0,0,4160749568,4294967295,31,0,0,4294705152,255,0,0,0,4227858432,4294967295,31,0,0,4294836224,511,0,0,0,4227858432,4294967295,31,0,0,4294836224,1023,0,0,0,4261412864,4294967295,31,0,0,4294901760,2047,0,0,0,4278190080,4294967295,31,0,0,4294901760,2047,0,0,0,4278190080,4294967295,31,0,0,4294934528,4095,0,0,0,4286578688,4294967295,31,0,0,4294934528,8191,0,0,0,4286578688,4294967295,31,0,0,4294934528,16383,0,0,0,4290772992,4294967295,31,0,0,4294950912,32767,0,0,0,4292870144,4294967295,31,0,0,4294950912,32767,0,0,0,4292870144,4294967295,31,0,0],
  [4294963200,4294967295,15,0,0,0,0,3221225472,4294967295,4095,4294963200,4294967295,15,0,0,0,0,3221225472,4294967295,2047,4294963200,4294967295,31,0,0,0,0,2147483648,4294967295,1023,4294965248,4294967295,31,0,0,0,0,2147483648,4294967295,1023,4294965248,4294967295,31,0,0,0,0,2147483648,4294967295,511,4294965248,4294967295,63,0,0,0,0,0,4294967295,255,4294965248,4294967295,63,0,0,0,0,0,4294967295,255,4294965248,4294967295,63,0,0,0,0,0,4294967294,127,4294965248,4294967295,127,0,0,0,0,0,4294967294,63,4294965248,4294967295,127,0,0,0,0,0,4294967292,63,4294965248,4294967295,127,0,0,0,0,0,4294967292,31,4294965248,4294967295,127,0,0,0,0,0,4294967288,15,4294965248,4294967295,255,0,0,0,0,0,4294967288,15,4294965248,4294967295,255,0,0,0,0,0,4294967280,7,4294965248,4294967295,255,0,0,0,0,0,4294967280,3,4294965248,4294967295,255,0,0,0,0,0,4294967264,3,4294965248,4294967295,255,0,0,0,0,0,4294967264,1,4294965248,4294967295,255,0,0,0,0,0,4294967232,0,4294963200,4294967295,255,0,0,0,0,0,4294967168,0,4294963200,4294967295,255,0,0,0,0,0,2147483520,0,4294963200,4294967295,255,0,0,0,0,0,1073741568,0,4294963200,4294967295,255,0,0,0,0,0,1073741312,0,4294963200,4294967295,255,0,0,0,0,0,536869888,0,4294963200,4294967295,255,0,0,0,0,0,268433408,0,4294963200,4294967295,255,0,0,0,0,0,134213632,0,4294963200,4294967295,255,0,0,0,0,0,67100672,0,4294963200,4294967295,255,0,0,0,0,0,16744448,0,4294963200,4294967295,127,0,0,0,0,0,3932160,0,4294963200,4294967295,127,0,0,0,0,0,0,0,4294963200,4294967295,127,0,0,0,0,0,0,0,4294963200,4294967295,63,0,0,0,0,0,0,0,4294963200,4294967295,63,0,0,0,0,0,0,0,4294959104,4294967295,31,0,0,0,0,0,0,0,4294959104,4294967295,31,0,0,0,0,0,0,0,4294959104,4294967295,15,0,0,0,0,0,0,0,4294959104,4294967295,7,0,0,0,0,0,0,0,4294959104,4294967295,7,0,0,0,0,0,0,0,4294950912,4294967295,3,0,0,0,0,0,0,0,4294950912,4294967295,1,0,0,0,0,0,0,0,4294950912,4294967295,0,0,0,0,0,0,0,0,4294934528,2147483647,0,0,0,0,0,0,0,0,4294934528,1073741823,0,0,0,0,0,0,0,0,4294901760,536870911,0,0,0,0,0,0,0,0,4294901760,134217727,0,0,0,0,0,0,0,0,4294836224,67108863,0,0,0,0,0,0,0,0,4294705152,16777215,0,0,0,0,0,0,0,0,4294443008,4194303,0,0,0,0,0,0,0,0,4292870144,524287,0,0,0,0,0,0,0,0,4286578688,65535,0,0,0,0,0,0,0,0,4227858432,2047,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,16380,0,0,0,0,0,0,0,0,0,65535,0,0,0,0,0,0,0,0,3221225472,262143,0,0,0,0,0,0,0,0,4026531840,1048575,0,0,0,0,0,0,0,0,4160749568,2097151,0,0,0,0,0,0,0,0,4227858432,4194303,0,0,0,0,0,0,0,0,4261412864,8388607,0,0,0,0,917504,0,0,0,4278190080,16777215,0,0,0,0,4161536,0,0,0,4286578688,33554431,0,0,0,0,16760832,0,0,0,4290772992,67108863,0,0,0,0,33546240,0,0,0,4292870144,67108863,0,0,0,0,67100672,0,0,0,4292870144,134217727,0,0,0,0,67100672,0,0,0,4293918720,134217727,0,8372224,0,0,134213632,0,0,0,4294443008,268435455,0,67106816,0,0,134213632,0,0,0,4294443008,268435455,0,268435200,0,0,268431360,0,0,0,4294705152,268435455,0,536870784,0,0,268431360,0,0,0,4294705152,536870911,0,1073741792,0,0,268431360,0,0,0,4294836224,536870911,0,2147483632,0,0,268431360,0,0,0,4294836224,536870911,0,4294967280,0,0,268431360,0,0,0,4294901760,268435455,0,4294967288,1,0,268427264,0,0,0,4294901760,268435455,0,4294967292,1,0,268427264,0,0,0,4294934528,268435455,0,4294967292,3,0,134209536,0,0,0,4294934528,268435455,0,4294967292,3,0,134201344,0,0,0,4294950912,134217727,0,4294967292,7,0,134201344,0,0,0,4294950912,134217727,0,4294967294,7,0,67076096,0,0,0,4294959104,67108863,0,4294967294,7,0,33488896,0,0,0,4294959104,67108863,0,4294967292,7,0,16646144,0,0,0,4294959104,33554431,0,4294967292,15,0,1048576,0,0,0,4294963200,16777215,0,4294967292,15,0,0,0,0,0,4294963200,16777215,0,4294967292,15,0,0,0,0,0,4294965248,8388607,0,4294967292,15,0,0,0,0,0,4294965248,4194303,0,4294967288,15,0,0,0,0,0,4294966272,2097151,0,4294967288,15,0,0,0,0,0,4294966272,1048575,0,4294967280,15,0,0,0,0,0,4294966784,524287,0,4294967280,15,0,0,0,0,0,4294966784,262143,0,4294967264,15,0,0,0,0,0,4294967040,262143,0,4294967264,15,0,0,0,0,0,4294967040,131071,0,4294967232,15,0,0,0,0,0,4294967168,65535,0,4294967232,15,0,0,0,0,0,4294967232,32767,0,4294967168,15,0,0,0,0,0,4294967232,16383,0,4294967040,7,0,0,0,0,0,4294967264,8191,0,4294967040,7,0,0,0,0,0,4294967264,4095,0,4294966784,7,0,0,0,0,0,4294967280,2047,0,4294966784,7,0,0,0,0,0,4294967288,2047,0,4294966272,7,0,0,0,0,0,4294967292,1023,0,4294965248,3,0,0,0,0,0,4294967292,511,0,4294965248,3,0,0,0,0,0,4294967294,511,0,4294963200,3,0,0,0,0,0,4294967295,255,0,4294963200,3,0,0,0,0,2147483648,4294967295,127,0,4294959104,1,0,0,0,0,3221225472,4294967295,127,0,4294950912,1,0,0,0,0,3221225472,4294967295,63,0,4294950912,1,0,0,0,0,3758096384,4294967295,63,0,4294934528,1,0,0,0,0,4026531840,4294967295,31,0,4294934528,0,0,0,0,0,4160749568,4294967295,31,0,4294901760,0,0,0,0,0,4160749568,4294967295,15,0,2147352576,0,0,0,0,0,4227858432,4294967295,15,0,2147221504,0,0,0,0,0,4261412864,4294967295,7,0,1073479680,0,0,0,0,0,4261412864,4294967295,7,0,1073217536,0,0,0,0,0,4278190080,4294967295,3,0,535822336,0,0,0,0,0,4278190080,4294967295,3,0,266338304,0,0,0,0,0,4286578688,4294967295,3,0,50331648,0,0,0,0,0,4286578688,4294967295,1,0,0,0,0,0,0,0,4290772992,4294967295,1,0,0,0,0,0,0,0,4290772992,4294967295,0,0,0,0,0,0,0,0,4290772992,4294967295,0,0,0,0,0,0,0,0,4292870144,2147483647,0,0,0,0,0,0,0,0,4292870144,2147483647,0,0,0,0,0,0,0,0,4292870144,1073741823,0,0,0,0,0,0,0,0,4292870144,1073741823,0,0,0,0,0,0,0,0,4292870144,536870911,0,0,0,0,0,0,0,0,4292870144,536870911,0,0,0,0,0,0,0,0,4292870144,268435455,0,0,0,0,0,0,0,0,4290772992,134217727,0,0,0,0,0,0,0,0,4290772992,134217727,0,0,0,0,0,0,0,0,4290772992,67108863,0,0,0,0,0,0,0,0,4286578688,33554431,0,0,0,0,0,0,0,0,4286578688,16777215,0,0,0,0,0,0,0,0,4278190080,8388607,0,0,0,0,0,0,0,0,4261412864,2097151,0,0,0,0,0,0,0,0,4227858432,1048575,0,0,0,0,0,0,0,0,4026531840,262143,0,0,0,0,0,0,0,0,3221225472,65535,0,0,0,0,0,0,0,0,0,4094,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1073479680,0,0,0,0,0,0,0,0,0,4294950912,3,0,0,0,0,0,0,0,0,4294963200,15,0,0,0,0,0,0,0,0,4294966272,63,0,0,0,0,0,0,0,0,4294967040,127,0,0,0,0,0,0,0,0,4294967168,255,0,0,0,0,0,0,0,0,4294967232,511,0,0,0,0,0,0,0,0,4294967264,1023,0,0,0,0,0,0,0,0,4294967280,1023,0,0,0,0,0,126976,0,0,4294967288,2047,0,0,0,0,0,260096,0,0,4294967288,2047,0,0,0,0,0,523264,0,0,4294967292,2047,0,0,0,0,0,1048064,0,0,4294967294,2047,0,0,0,0,0,1048064,0,0,4294967294,2047,0,0,0,0,0,2096896,0,0,4294967295,2047,0,0,0,0,0,2096896,0,0,4294967295,2047,0,0,0,0,0,2096896,0,0,4294967295,2047,0,0,0,0,0,2096896,0,2147483648,4294967295,1023,0,0,0,0,0,2096896,0,2147483648,4294967295,1023,0,0,0,0,0,2096896,0,2147483648,4294967295,1023,0,0,0,0,0,2096896,0,2147483648,4294967295,511,0,0,0,0,0,2096640,0,3221225472,4294967295,255,0,0,0,0,0,1048064,0,3221225472,4294967295,255,0,0,0,0,0,1047552,0,3221225472,4294967295,127,0,0,0,0,0,522240,0,3221225472,4294967295,127,0,0,0,0,0,258048,0,3221225472,4294967295,63,0,0,0,0,0,49152,0,3221225472,4294967295,31,0,0,0,0,0,0,0,3221225472,4294967295,31,0,0,0,0,0,0,0,3758096384,4294967295,15,0,0,0,0,0,0,0,3758096384,4294967295,15,0,0,0,0,0,0,0,3758096384,4294967295,7,0,0,0,0,0,0,0,3758096384,4294967295,3,0,0,0,0,0,0,0,3758096384,4294967295,3,0,0,0,0,0,0,0,3758096384,4294967295,1,0,0,0,0,0,0,0,3758096384,4294967295,0,0,0,0,0,0,0,0,3758096384,4294967295,0,0,0,0,0,0,0,0,3758096384,2147483647,0,0,0,0,0,0,0,0,3758096384,1073741823,0,0,0,0,0,0,0,0,3758096384,1073741823,0,0,0,0,0,0,0,0,3221225472,536870911,0,0,0,0,0,0,0,0,3221225472,268435455,0,0,0,0,0,0,0,0,3221225472,268435455,0,0,0,0,0,0,0,0,3221225472,134217727,0,0,0,0,0,0,0,0,3221225472,134217727,0,0,0,0,0,0,0,0,3221225472,67108863,0,0,62914560,0,0,0,0,0,3221225472,33554431,0,0,1073479680,0,0,0,0,0,3221225472,33554431,0,0,4294934528,0,0,0,0,0,3221225472,16777215,0,0,4294959104,3,0,0,0,0,3221225472,16777215,0,0,4294963200,7,0,0,0,0,2147483648,8388607,0,0,4294966272,15,0,0,0,0,2147483648,4194303,0,0,4294966784,31,0,0,0,0,2147483648,2097151,0,0,4294967168,63,0,0,0,0,2147483648,2097151,0,0,4294967232,127,0,0,0,0,0,1048575,0,0,4294967280,255,0,0,0,0,0,524287,0,0,4294967288,511,0,0,0,0,0,262142,0,0,4294967294,1023,0,0,0,0,0,131068,0,1,4294967295,1023,0,0,0,0,0,65532,0,3221225487,4294967295,2047,0,0,0,0,0,32760,0,4026531903,4294967295,4095,0,0,0,0,0,8160,0,4227858943,4294967295,4095,0,0,0,0,0,896,0,4286586879,4294967295,8191,0,0,0,0,0,0,0,4294967295,4294967295,16383,0,0,0,0,0,0,0,4294967295,4294967295,16383,0,0,0,0,0,0,0,4294967295,4294967295,32767,0,0,0,0,0,0,0,4294967295,4294967295,32767,0,0,0,0,0,0,0,4294967295,4294967295,65535,0,0,0,0,0,0,0,4294967295,4294967295,65535,0,0,0,0,0,0,0,4294967295,4294967295,65535,0,0,0,0,0,0,0,4294967295,4294967295,131071,0,0,0,0,0,0,0,4294967295,4294967295,131071,0,0,0,0,0,0,0,4294967295,4294967295,262143,0,0,0,0,0,0,0,4294967295,4294967295,262143,0,0,0,0,0,0,0,4294967295,4294967295,262143,0,0,0,0,0,0,0,4294967295,4294967295,262143,0,0,0,0,0,0,0,4294967295,4294967295,524287,0,0,0,0,0,0,0,4294967295,4294967295,524287,0,0,0,0,0,0,0,4294967295,4294967295,524287,0,0,0,0,0,0,0,4294967295,4294967295,524287,0,0,0,0,0,0,0,4294967295,4294967295,524287,0,0,0,0,0,0,0,4294967295,4294967295,524287,0,0,0,0,0,0,0,4294967295,4294967295,524287,0,0,0,0,0,0,0,4294967295,4294967295,262143,0,0,0,0,0,0,0,4294967295,4294967295,262143,0,0,0,0,0,0,0,4294967295,4294967295,131071,0,0,0,0,0,0,0,4294967295,4294967295,65535,0,0,0,0,0,0,0,4294967295,4294967295,8191,0,0,0,0,0,0,0,4294967295,16383,0,0,0,0,0,0,0,0,4294967295,511,0,0,0,0,0,0,0,0,4294967295,127,0,0,0,0,0,0,0,0,4294967295,31,0,0,0,0,0,0,0,0,4294967295,7,0,0,0,0,0,0,0,0,4294967295,3,0,0,0,0,0,0,0,0,4294967295,0,0,0,0,0,0,0,0,0,2147483647,0,0,0,0,0,0,0,0,0,1073741823,0,0,0,0,0,0,0,0,0,536870911,0,0,0,0,0,0,0,0,0,268435455,0,0,0,0,0,0,0,0,0,134217727,0,0,0,0,0,0,0,0,0,134217727,0,0,0,0,0,0,0,0,0,67108863,0,0,0,0,0,0,0,0,0,33554431,0,0,0,0,0,0,0,0,0,16777215,0,0,0,0,0,0,0,0,0,8388607,0,0,0,0,0,0,0,0,0,8388607,0,0,0,0,0,0,0,0,0,4194303,0,0,0,0,0,0,0,0,0,2097151,0,0,0,0,0,0,0,0,0,2097151,0,0,0,0,0,0,0,0,0,1048575,0,0,0,0,0,0,0,0,0,524287,0,0,0,0,0,0,0,0,0,262143,0,0,0,0,0,0,0,0,0,262143,0,0,0,0,0,0,0,0,0,131071,0,0,0,0,0,0,0,0,0,65535,0,0,0,0,0,0,0,0,0,32767,0,0,0,0,0,0,0,0,0,32767,0,0,0,0,0,0,0,0,0,16383,0,0,0,0,0,0,0,0,0,8191,0,0,0,0,0,0,0,0,0,4095,0,0,0,0,0,0,0,0,0,4095,0,0,0,0,0,0,0,0,0,2047,0,0,0,0,0,0,0,0,0,1023,0,0,0,0,0,0,0,0,0,511,0,0,0,0,0,0,0,0,0,255,0,0,0,0,0,0,0,0,0,255,0,0,0,0,0,0,0,0,0,127,0,0,0,0,0,0,0,0,0,63,0,0,0,0,0,0,0,0,0,31,0,0,0,0,0,0,0,0,0,15,0,0,0,0,0,0,0,0,0,7,0,0,0,0,0,0,0,0,0,3,0,0,0,0,0,0,0,0,0,1,0,0,0,0,62914560,0,0,0,0,0,0,0,0,0,267386880,0,0,0,0,0,0,0,0,0,1073217536,0,0,0,0,0,0,0,0,0,2147221504,0,0,0,0,0,0,0,0,0,4294836224,0,0,0,0,0,0,0,0,0,4294836224,1,0,0,0,0,0,0,0,0,4294901760,3,0,0,0,0,0,0,0,0,4294901760,3,0,0,0,0,0,0,0,0,4294934528,7,0,0,0,0,0,0,0,0,4294934528,15,0,0,0,0,0,0,0,0,4294934528,15,0,0,0,0,0,0,0,0,4294934528,31,0,0,0,0,0,0,0,0,4294950912,63,0,0,0,0,0,0,0,0,4294950912,63,0,0,0,0,0,0,0,0,4294950912,127,0,0,0,0,0,0,0,0,4294950912,127,0,0,0,0,0,0,0,0,4294950912,255,0,0,0,0,0,0,0,0,4294950912,255,0,0,0,0,0,0,0,0,4294950912,511,0,0,0,0,0,0,0,0,4294950912,1023,0,0,0,0,0,0,0,0,4294950912,1023,0,0,0,0,0,0,0,0,4294950912,2047,0,0,0,0,0,0,0,0,4294950912,2047,0,0,0,0,0,0,0,0,4294950912,4095,0,0,0,0,0,0,0,0,4294950912,4095,0,1073676288,0,0,0,0,0,0,4294950912,8191,0,4294959104,1,0,0,0,0,0,4294950912,16383,0,4294966272,15,0,0,0,0,0,4294950912,16383,0,4294967040,63,0,0,0,14,0,4294959104,32767,0,4294967232,255,0,0,0,63,0,4294959104,65535,0,4294967280,1023,0,0,2147483648,127,0,4294959104,65535,0,4294967288,4095,0,0,3221225472,127,0,4294959104,131071,0,4294967294,16383,0,0,3221225472,255,0,4294959104,262143,0,4294967295,65535,0,0,3758096384,255,0,4294959104,262143,3221225472,4294967295,262143,0,0,3758096384,511,0,4294959104,524287,3758096384,4294967295,2097151,0,0,4026531840,511,0,4294959104,1048575,4026531840,4294967295,8388607,0,0,4026531840,511,0,4294959104,2097151,4160749568,4294967295,67108863,0,0,4026531840,1023,0,4294959104,2097151,4261412864,4294967295,536870911,0,0,4160749568,1023,0,4294959104,4194303,4278190080,4294967295,4294967295,0,0,4160749568,1023,0,4294963200,8388607,4286578688,4294967295,4294967295,0,0,4160749568,1023,0,4294963200,16777215,4290772992,4294967295,4294967295,0,0,4160749568,1023,0,4294963200,33554431,4292870144,4294967295,4294967295,0,0,4227858432,2047,0,4294963200,33554431,4293918720,4294967295,4294967295,0,0,4227858432,2047,0,4294963200,67108863,4294443008,4294967295,4294967295,0,0,4227858432,2047,0,4294963200,134217727,4294705152,4294967295,4294967295,0,0,4227858432,2047,0,4294965248,268435455,4294836224,4294967295,4294967295,0,0,4227858432,2047,0,4294965248,536870911,4294901760,4294967295,4294967295,0,0,4227858432,2047,0,4294965248,536870911,4294901760,4294967295,4294967295,0,0,4227858432,2047,0,4294965248,1073741823,4294934528,4294967295,4294967295,0,0,4227858432,2047,0,4294965248,2147483647,4294950912,4294967295,4294967295,0,0,4227858432,2047,0,4294966272,4294967295,4294950912,4294967295,4294967295,0,0,4227858432,2047,0,4294966272,4294967295,4294950912,4294967295,4294967295,0,0,4227858432,2047,0,4294966272,4294967295,4294959105,4294967295,4294967295,0,0,4227858432,2047,0,4294966272,4294967295,4294959107,4294967295,4294967295,0,0,4227858432,1023,0,4294966784,4294967295,4294959107,4294967295,4294967295,0,0,4160749568,1023,0,4294966784,4294967295,4294959107,4294967295,4294967295,0,0,4160749568,1023,0,4294966784,4294967295,4294950919,4294967295,4294967295,0,0,4026531840,511,0,4294967040,4294967295,4294934535,4294967295,4294967295,0,0,4026531840,255,0,4294967040,4294967295,4294901763,4294967295,4294967295,0,0,3758096384,127,0,4294967040,4294967295,4294836227,4294967295,4294967295,0,0,3221225472,63,0,4294967168,4294967295,4294443011,4294967295,4294967295,0,0,0,14,0,4294967168,4294967295,4290772993,4294967295,4294967295,0,0,0,0,0,4294967168,4294967295,4227858433,4294967295,4294967295,0,0,0,0,0,4294967232,4294967295,3758096384,4294967295,4294967295,0,0,0,0,0,4294967232,4294967295,0,4294967295,4294967295,0,0,0,0,0,4294967264,2147483647,0,4294967280,4294967295,0,0,0,0,0,4294967264,2147483647,0,4294967168,4294967295,0,0,0,0,0,4294967280,1073741823,0,4294966784,4294967295,0,0,0,0,0,4294967280,536870911,0,4294963200,4294967295,0,0,0,0,0,4294967288,536870911,0,4294934528,4294967295,0,0,0,0,0,4294967292,268435455,0,4294836224,4294967295,0,0,0,0,0,4294967294,134217727,0,4294443008,4294967295,0,0,0,0,0,4294967294,134217727,0,4292870144,4294967295,0,0,0,0,0,4294967295,67108863,0,4286578688,4294967295,0,0,0,0,3221225472,4294967295,33554431,0,4261412864,4294967295,0,0,0,0,3758096384,4294967295,16777215,0,4227858432,4294967295,0,0,0,0,4160749568,4294967295,8388607,0,4026531840,4294967295,0,0,0,0,4261412864,4294967295,4194303,0,3758096384,4294967295,0,0,0,0,4290772992,4294967295,2097151,0,2147483648,4294967295,0,0,0,0,4294443008,4294967295,1048575,0,0,4294967295,0,0,0,0,4294901760,4294967295,524287,0,0,4294967294,0,0,0,0,4294950912,4294967295,262143,0,0,4294967292,0,0,0,0,4294963200,4294967295,65535,0,0,4294967280,0,0,0,0,4294965248,4294967295,32767,0,0,4294967264,0,0,0,0,4294966784,4294967295,8191,0,0,4294967232,0,4193792,0,0,4294967040,4294967295,2047,0,0,4294967168,0,67108856,0,0,4294967168,4294967295,255,0,0,4294967040,0,268435455,0,0,4294967168,4294967295,15,0,0,4294966784,3758096384,536870911,0,0,4294967232,1073741823,0,0,0,4294966272,4160749568,2147483647,0,0,4294967232,4194303,0,0,0,4294965248,4261412864,2147483647,0,0,4294967264,131071,0,0,0,4294963200,4286578688,4294967295,0,0,4294967264,16383,0,0,0,4294959104,4290772992,4294967295,1,0,4294967264,4095,0,0,0,4294950912,4293918720,4294967295,1,0,4294967280,1023,0,0,0,4294950912,4294443008,4294967295,3,0,4294967280,511,0,0,0,4294934528,4294705152,4294967295,3,0,4294967280,255,0,0,0,4294901760,4294836224,4294967295,7,0,4294967280,127,0,0,0,4294836224,4294901760,4294967295,7,0,4294967280,63,0,0,0,4294705152,4294901760,4294967295,7,0,4294967280,63,0,0,0,4294443008,4294934528,4294967295,7,0,4294967280,31,0,0,0,4293918720,4294950912,4294967295,15,0,4294967280,15,0,0,0,4292870144,4294950912,4294967295,15,0,4294967280,15,0,0,0,4290772992,4294959104,4294967295,15,0,4294967280,7,0,0,0,4286578688,4294959104,4294967295,15,0,4294967280,3,0,0,0,4278190080,4294963200,4294967295,15,0,4294967264,3,0,0,0,4261412864,4294963200,4294967295,15,0,4294967264,1,0,0,0,4227858432,4294965248,4294967295,15,0,4294967264,1,0,0,0,4227858432,4294965248,4294967295,31,0,4294967264,0,0,0,0,4160749568,4294965248,4294967295,31,0,4294967264,0,0,0,0,4026531840,4294966272,4294967295,31,0,2147483584,0,0,0,0,3758096384,4294966272,4294967295,31,0,1073741760,0,0,0,0,3221225472,4294966272,4294967295,31,0,1073741760,0,0,0,0,2147483648,4294966272,4294967295,31,0,536870784,0,0,0,0,0,4294966272,4294967295,31,0,536870784,0,0,0,0,0,4294966272,4294967295,31,0,268435328,0,0,0,0,0,4294966272,4294967295,31,0,134217472,0,0,0,0,0,4294966272,4294967295,31,0,134217472,0,0,0,0,0,4294966272,4294967295,31,0,67108352,0,0,0,0,0,4294966272,4294967295,31,0,33553920,0,0,0,0,0,4294966272,4294967295,31,0,16776192,0,0,0,0,0,4294966272,4294967295,31,0,16775168,0,0,0,0,0,4294966272,4294967295,31,0,4190208,0,0,0,0,0,4294966272,4294967295,31,0,2088960,0,0,0,0,0,4294966272,4294967295,31,0,491520,0,0,0,0,0,4294966272,4294967295,31,0,0,0,0,0,0,0,4294966272,4294967295,31,0,0,0,0,0,0,0,4294966272,4294967295,31,0,0,0,0,0,0,0,4294965248,4294967295,31,0,0,0,0,0,0,0,4294965248,4294967295,31,0,0,0,0,0,0,0,4294965248,4294967295,31,0,0,0,0,0,0,0,4294965248,4294967295,31,0,0,0,0,0,0,0,4294965248,4294967295,31,0,0,0,0,0,0,0,4294963200,4294967295,63,0,0,0,0,0,0,0,4294963200,4294967295,63,0,0,0,0,0,0,0],
  [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
];
