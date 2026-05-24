import express from "express";

const app = express();

function getItems(req: express.Request, res: express.Response): void {
  res.json([]);
}

function createItem(req: express.Request, res: express.Response): void {
  res.status(201).json(req.body);
}

function corsMiddleware(
  req: express.Request,
  res: express.Response,
  next: express.NextFunction,
): void {
  res.setHeader("Access-Control-Allow-Origin", "*");
  next();
}

app.get("/api/items/:id", getItems);
app.post("/api/items", createItem);
app.use(corsMiddleware);

app.listen(3000, () => {
  console.log("Server running on port 3000");
});
